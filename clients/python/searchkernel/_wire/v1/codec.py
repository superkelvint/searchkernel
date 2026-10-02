"""Private V1 envelope codec for the stable Python client model."""

from __future__ import annotations

import importlib
import inspect
from functools import lru_cache
from typing import Any

import flatbuffers

from searchkernel._wire.v1.generated.SearchKernel.Protocol import RequestEnvelope, RequestPayload
from searchkernel._wire.v1.generated.SearchKernel.Protocol import ResponseEnvelope, ResponsePayload
from searchkernel._wire.v1.generated.SearchKernel.Wire.IndexTarget import IndexTargetT  # noqa: E402
from searchkernel._wire.v1.generated.semantic_metadata import ENUM_FIELDS
from ..._semantic import SemanticEnumValue, SemanticTable


PROTOCOL_VERSION = 1
REQUEST_IDENTIFIER = b"SKRQ"
RESPONSE_IDENTIFIER = b"SKRS"
_WIRE_PACKAGE = "searchkernel._wire.v1.generated.SearchKernel.Wire"


@lru_cache(maxsize=None)
def _enum_maps(enum_name: str) -> tuple[dict[str, int], dict[int, str]]:
    module = importlib.import_module(f"{_WIRE_PACKAGE}.{enum_name}")
    enum_type = getattr(module, enum_name)
    by_name = {
        name: int(value)
        for name, value in vars(enum_type).items()
        if not name.startswith("_") and isinstance(value, int)
    }
    by_value: dict[int, str] = {}
    for name, value in by_name.items():
        if value in by_value:
            raise ValueError(f"wire enum {enum_name} has duplicate value {value}")
        by_value[value] = name
    return by_name, by_value


def _semantic_enum_to_wire(value: SemanticEnumValue) -> int:
    by_name, _ = _enum_maps(value.enum_name)
    try:
        return by_name[value.member_name]
    except KeyError as exc:
        raise ValueError(
            f"wire enum {value.enum_name} has no member {value.member_name}"
        ) from exc


def wire_enum_matches(value: Any, expected: SemanticEnumValue) -> bool:
    if value == expected:
        return True
    return isinstance(value, int) and value == _semantic_enum_to_wire(expected)


def _wire_enum_to_semantic(enum_name: str, value: int) -> SemanticEnumValue:
    _, by_value = _enum_maps(enum_name)
    try:
        member_name = by_value[int(value)]
    except KeyError as exc:
        raise ValueError(f"wire enum {enum_name} has no value {value}") from exc
    return SemanticEnumValue(enum_name, member_name)


def _convert_enum_to_wire(spec: str, value: Any) -> Any:
    if spec.endswith("[]"):
        if value is None:
            return None
        if not isinstance(value, (list, tuple)):
            raise TypeError(f"semantic enum vector {spec} must be a sequence")
        enum_name = spec[:-2]
        converted = []
        for item in value:
            if not isinstance(item, SemanticEnumValue):
                raise TypeError(f"semantic enum vector {spec} requires SemanticEnumValue items")
            if item.enum_name != enum_name:
                raise TypeError(
                    f"semantic enum mismatch: expected {enum_name}, got {item.enum_name}"
                )
            converted.append(_semantic_enum_to_wire(item))
        return converted
    if value is None:
        return None
    if not isinstance(value, SemanticEnumValue):
        raise TypeError(f"semantic enum {spec} requires a SemanticEnumValue")
    if value.enum_name != spec:
        raise TypeError(
            f"semantic enum mismatch: expected {spec}, got {value.enum_name}"
        )
    return _semantic_enum_to_wire(value)


def _convert_enum_from_wire(spec: str, value: Any) -> Any:
    if spec.endswith("[]"):
        if value is None:
            return None
        enum_name = spec[:-2]
        return [_wire_enum_to_semantic(enum_name, item) for item in value]
    if value is None:
        return None
    return _wire_enum_to_semantic(spec, value)


def _to_generated(value: Any) -> Any:
    if isinstance(value, SemanticEnumValue):
        return _semantic_enum_to_wire(value)
    if isinstance(value, SemanticTable):
        module = importlib.import_module(f"{_WIRE_PACKAGE}.{value._schema_name}")
        constructor = getattr(module, f"{value._schema_name}T")
        parameters = [
            parameter.name
            for parameter in inspect.signature(constructor).parameters.values()
            if parameter.kind in (parameter.POSITIONAL_ONLY, parameter.POSITIONAL_OR_KEYWORD)
        ]
        enum_fields = ENUM_FIELDS.get(value._schema_name, {})
        fields = {
            key: (
                _convert_enum_to_wire(enum_fields[key], item)
                if key in enum_fields
                else _to_generated(item)
            )
            for key, item in value._values.items()
        }
        for key, item in zip(parameters, value._positional):
            converted = (
                _convert_enum_to_wire(enum_fields[key], item)
                if key in enum_fields
                else _to_generated(item)
            )
            fields.setdefault(key, converted)
        return constructor(**fields)
    if isinstance(value, list):
        return [_to_generated(item) for item in value]
    if isinstance(value, tuple):
        return tuple(_to_generated(item) for item in value)
    if isinstance(value, dict):
        return {key: _to_generated(item) for key, item in value.items()}
    if hasattr(value, "Pack") and hasattr(value, "__dict__"):
        return value.__class__(
            **{key: _to_generated(item) for key, item in vars(value).items()}
        )
    return value


def _from_generated(value: Any) -> Any:
    if isinstance(value, list):
        return [_from_generated(item) for item in value]
    if isinstance(value, tuple):
        return tuple(_from_generated(item) for item in value)
    if hasattr(value, "__dict__") and value.__class__.__name__.endswith("T"):
        table_name = value.__class__.__name__[:-1]
        enum_fields = ENUM_FIELDS.get(table_name, {})
        fields = {
            key: (
                _convert_enum_from_wire(enum_fields[key], item)
                if key in enum_fields
                else _from_generated(item)
            )
            for key, item in vars(value).items()
        }
        return SemanticTable(table_name, **fields)
    return value

_REQUEST_TYPES = {
    "list_indexes": RequestPayload.RequestPayload.SearchKernel_Wire_ListIndexesRequest,
    "insert": RequestPayload.RequestPayload.SearchKernel_Wire_InsertRequest,
    "upsert": RequestPayload.RequestPayload.SearchKernel_Wire_UpsertRequest,
    "patch": RequestPayload.RequestPayload.SearchKernel_Wire_PatchRequest,
    "update": RequestPayload.RequestPayload.SearchKernel_Wire_UpdateRequest,
    "delete": RequestPayload.RequestPayload.SearchKernel_Wire_DeleteRequest,
    "get": RequestPayload.RequestPayload.SearchKernel_Wire_GetRequest,
    "search": RequestPayload.RequestPayload.SearchKernel_Wire_SearchRequest,
    "analyze": RequestPayload.RequestPayload.SearchKernel_Wire_AnalyzeRequest,
    "describe": RequestPayload.RequestPayload.SearchKernel_Wire_DescribeRequest,
    "inspect_query": RequestPayload.RequestPayload.SearchKernel_Wire_InspectQueryRequest,
    "create_index": RequestPayload.RequestPayload.SearchKernel_Wire_CreateIndexRequest,
    "open_index": RequestPayload.RequestPayload.SearchKernel_Wire_OpenIndexRequest,
    "delete_index": RequestPayload.RequestPayload.SearchKernel_Wire_DeleteIndexRequest,
}
_RESPONSE_TYPES = {
    "list_indexes": ResponsePayload.ResponsePayload.SearchKernel_Wire_ListIndexesResult,
    "insert": ResponsePayload.ResponsePayload.SearchKernel_Wire_InsertResult,
    "upsert": ResponsePayload.ResponsePayload.SearchKernel_Wire_UpsertResult,
    "patch": ResponsePayload.ResponsePayload.SearchKernel_Wire_PatchResult,
    "update": ResponsePayload.ResponsePayload.SearchKernel_Wire_UpdateResult,
    "delete": ResponsePayload.ResponsePayload.SearchKernel_Wire_DeleteResult,
    "get": ResponsePayload.ResponsePayload.SearchKernel_Wire_GetResult,
    "search": ResponsePayload.ResponsePayload.SearchKernel_Wire_SearchResult,
    "analyze": ResponsePayload.ResponsePayload.SearchKernel_Wire_AnalyzeResult,
    "describe": ResponsePayload.ResponsePayload.SearchKernel_Wire_DescribeResult,
    "inspect_query": ResponsePayload.ResponsePayload.SearchKernel_Wire_InspectQueryResult,
    "create_index": ResponsePayload.ResponsePayload.SearchKernel_Wire_CreateIndexResult,
    "open_index": ResponsePayload.ResponsePayload.SearchKernel_Wire_OpenIndexResult,
    "delete_index": ResponsePayload.ResponsePayload.SearchKernel_Wire_DeleteIndexResult,
}
ERROR_TYPE = ResponsePayload.ResponsePayload.SearchKernel_Wire_Error


def pack_object(obj: Any, *, file_identifier: bytes | None = None) -> bytes:
    """Pack one generated object-api table as a FlatBuffers root."""

    builder = flatbuffers.Builder(1024)
    offset = _to_generated(obj).Pack(builder)
    if file_identifier is None:
        builder.Finish(offset)
    else:
        builder.Finish(offset, file_identifier=file_identifier)
    return bytes(builder.Output())


def pack_request(
    operation: str,
    payload: Any,
    *,
    target: str | None = None,
    expected_index_generation: str | None = None,
) -> bytes:
    try:
        payload_type = _REQUEST_TYPES[operation]
    except KeyError as exc:
        raise ValueError(f"unsupported request operation: {operation}") from exc
    index_target = None
    if target is not None:
        if not isinstance(target, str):
            raise TypeError("Search Kernel target must be a string")
        if expected_index_generation is not None and (
            not isinstance(expected_index_generation, str) or not expected_index_generation
        ):
            raise ValueError("expected index generation must be a non-empty string")
        index_target = IndexTargetT(target, expected_index_generation)
    envelope = RequestEnvelope.RequestEnvelopeT(
        PROTOCOL_VERSION,
        payload_type,
        _to_generated(payload),
        index_target,
    )
    return pack_object(envelope, file_identifier=REQUEST_IDENTIFIER)


def unpack_response(data: bytes, operation: str) -> Any:
    if not ResponseEnvelope.ResponseEnvelope.ResponseEnvelopeBufferHasIdentifier(data, 0):
        raise ValueError("response is not a Search Kernel envelope")
    envelope = ResponseEnvelope.ResponseEnvelopeT.InitFromPackedBuf(data)
    if envelope.protocolVersion != PROTOCOL_VERSION:
        raise ValueError(f"unsupported Search Kernel protocol version: {envelope.protocolVersion}")
    expected = _RESPONSE_TYPES.get(operation)
    if expected is None:
        raise ValueError(f"unsupported response operation: {operation}")
    if envelope.payloadType not in {expected, ERROR_TYPE}:
        raise ValueError(
            f"unexpected response payload type {envelope.payloadType} for {operation}"
        )
    return _from_generated(envelope)


def expected_response_type(operation: str) -> int:
    return _RESPONSE_TYPES[operation]


__all__ = [
    "ERROR_TYPE",
    "PROTOCOL_VERSION",
    "REQUEST_IDENTIFIER",
    "RESPONSE_IDENTIFIER",
    "expected_response_type",
    "pack_object",
    "pack_request",
    "unpack_response",
]
