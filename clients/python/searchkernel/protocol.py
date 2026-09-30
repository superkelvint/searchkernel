"""Small framing helpers around the generated V2 FlatBuffers object API."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

import flatbuffers

_generated_root = Path(__file__).resolve().parent / "_wire_generated"
if str(_generated_root) not in sys.path:
    sys.path.insert(0, str(_generated_root))

from SearchKernel.Protocol.V2 import RequestEnvelope, RequestPayload  # noqa: E402
from SearchKernel.Protocol.V2 import ResponseEnvelope, ResponsePayload  # noqa: E402
from SearchKernel.V2.IndexTarget import IndexTargetT  # noqa: E402


PROTOCOL_VERSION = 2
REQUEST_IDENTIFIER = b"SKRQ"
RESPONSE_IDENTIFIER = b"SKRS"

_REQUEST_TYPES = {
    "insert": RequestPayload.RequestPayload.SearchKernel_V2_InsertRequest,
    "upsert": RequestPayload.RequestPayload.SearchKernel_V2_UpsertRequest,
    "patch": RequestPayload.RequestPayload.SearchKernel_V2_PatchRequest,
    "update": RequestPayload.RequestPayload.SearchKernel_V2_UpdateRequest,
    "delete": RequestPayload.RequestPayload.SearchKernel_V2_DeleteRequest,
    "get": RequestPayload.RequestPayload.SearchKernel_V2_GetRequest,
    "search": RequestPayload.RequestPayload.SearchKernel_V2_SearchRequest,
    "analyze": RequestPayload.RequestPayload.SearchKernel_V2_AnalyzeRequest,
    "describe": RequestPayload.RequestPayload.SearchKernel_V2_DescribeRequest,
    "inspect_query": RequestPayload.RequestPayload.SearchKernel_V2_InspectQueryRequest,
    "create_index": RequestPayload.RequestPayload.SearchKernel_V2_CreateIndexRequest,
    "open_index": RequestPayload.RequestPayload.SearchKernel_V2_OpenIndexRequest,
    "delete_index": RequestPayload.RequestPayload.SearchKernel_V2_DeleteIndexRequest,
}
_RESPONSE_TYPES = {
    "insert": ResponsePayload.ResponsePayload.SearchKernel_V2_InsertResult,
    "upsert": ResponsePayload.ResponsePayload.SearchKernel_V2_UpsertResult,
    "patch": ResponsePayload.ResponsePayload.SearchKernel_V2_PatchResult,
    "update": ResponsePayload.ResponsePayload.SearchKernel_V2_UpdateResult,
    "delete": ResponsePayload.ResponsePayload.SearchKernel_V2_DeleteResult,
    "get": ResponsePayload.ResponsePayload.SearchKernel_V2_GetResult,
    "search": ResponsePayload.ResponsePayload.SearchKernel_V2_SearchResult,
    "analyze": ResponsePayload.ResponsePayload.SearchKernel_V2_AnalyzeResult,
    "describe": ResponsePayload.ResponsePayload.SearchKernel_V2_DescribeResult,
    "inspect_query": ResponsePayload.ResponsePayload.SearchKernel_V2_InspectQueryResult,
    "create_index": ResponsePayload.ResponsePayload.SearchKernel_V2_CreateIndexResult,
    "open_index": ResponsePayload.ResponsePayload.SearchKernel_V2_OpenIndexResult,
    "delete_index": ResponsePayload.ResponsePayload.SearchKernel_V2_DeleteIndexResult,
}
ERROR_TYPE = ResponsePayload.ResponsePayload.SearchKernel_V2_Error


def pack_object(obj: Any, *, file_identifier: bytes | None = None) -> bytes:
    """Pack one generated object-api table as a FlatBuffers root."""

    builder = flatbuffers.Builder(1024)
    offset = obj.Pack(builder)
    if file_identifier is None:
        builder.Finish(offset)
    else:
        builder.Finish(offset, file_identifier=file_identifier)
    return bytes(builder.Output())


def pack_request(operation: str, payload: Any, *, target: str | None = None) -> bytes:
    try:
        payload_type = _REQUEST_TYPES[operation]
    except KeyError as exc:
        raise ValueError(f"unsupported V2 request operation: {operation}") from exc
    index_target = None
    if target is not None:
        if not isinstance(target, str):
            raise TypeError("Search Kernel target must be a string")
        index_target = IndexTargetT(target)
    envelope = RequestEnvelope.RequestEnvelopeT(PROTOCOL_VERSION, payload_type, payload, index_target)
    return pack_object(envelope, file_identifier=REQUEST_IDENTIFIER)


def unpack_response(data: bytes, operation: str) -> Any:
    if not ResponseEnvelope.ResponseEnvelope.ResponseEnvelopeBufferHasIdentifier(data, 0):
        raise ValueError("response is not a Search Kernel V2 envelope")
    envelope = ResponseEnvelope.ResponseEnvelopeT.InitFromPackedBuf(data)
    if envelope.protocolVersion != PROTOCOL_VERSION:
        raise ValueError(f"unsupported Search Kernel protocol version: {envelope.protocolVersion}")
    expected = _RESPONSE_TYPES.get(operation)
    if expected is None:
        raise ValueError(f"unsupported V2 response operation: {operation}")
    if envelope.payloadType not in {expected, ERROR_TYPE}:
        raise ValueError(
            f"unexpected response payload type {envelope.payloadType} for {operation}"
        )
    return envelope


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
