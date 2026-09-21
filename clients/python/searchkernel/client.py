"""Synchronous Python client for the canonical gRPC/V2 transport."""

from __future__ import annotations

import math
import os
import signal
import socket
import subprocess
import tempfile
import time
import weakref
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .grpc_transport import GrpcTransport, GrpcTransportError
from .protocol import ERROR_TYPE, pack_object, pack_request, unpack_response

from SearchKernel.V2.AnalyzeFieldTarget import AnalyzeFieldTargetT
from SearchKernel.V2.AnalyzeRequest import AnalyzeRequestT
from SearchKernel.V2.AnalyzeTarget import AnalyzeTarget as AnalyzeTargetKind
from SearchKernel.V2.BoolValue import BoolValueT
from SearchKernel.V2.BytesValue import BytesValueT
from SearchKernel.V2.DataType import DataTypeT
from SearchKernel.V2.DataTypeKind import DataTypeKind
from SearchKernel.V2.DeleteRequest import DeleteRequestT
from SearchKernel.V2.DescribeRequest import DescribeRequestT
from SearchKernel.V2.Document import DocumentT
from SearchKernel.V2.ErrorCode import ErrorCode
from SearchKernel.V2.FieldDefinition import FieldDefinitionT
from SearchKernel.V2.FieldProjection import FieldProjectionT
from SearchKernel.V2.Float64Value import Float64ValueT
from SearchKernel.V2.NonFiniteFloatValue import NonFiniteFloatValueT
from SearchKernel.V2.NonFiniteFloatKind import NonFiniteFloatKind
from SearchKernel.V2.NullValue import NullValue
from SearchKernel.V2.NullValueValue import NullValueValueT
from SearchKernel.V2.GetRequest import GetRequestT
from SearchKernel.V2.IndexDefinition import IndexDefinitionT
from SearchKernel.V2.InsertRequest import InsertRequestT
from SearchKernel.V2.Int64Value import Int64ValueT
from SearchKernel.V2.MatchingConfig import MatchingConfigT
from SearchKernel.V2.MatchingMode import MatchingMode
from SearchKernel.V2.OptionalBool import OptionalBoolT
from SearchKernel.V2.PatchRequest import PatchRequestT
from SearchKernel.V2.StringValue import StringValueT
from SearchKernel.V2.DenseTensorValues import DenseTensorValuesT
from SearchKernel.V2.SparseTensorCells import SparseTensorCellsT
from SearchKernel.V2.MixedTensorBlocks import MixedTensorBlocksT
from SearchKernel.V2.TensorPayloadKind import TensorPayloadKind
from SearchKernel.V2.TensorValue import TensorValueT
from SearchKernel.V2.UInt64Value import UInt64ValueT
from SearchKernel.V2.UpsertRequest import UpsertRequestT
from SearchKernel.V2.UpdateRequest import UpdateRequestT
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.ValueEntry import ValueEntryT
from SearchKernel.V2.ValueKind import ValueKind
from SearchKernel.V2.ValueList import ValueListT
from SearchKernel.V2.ValueObject import ValueObjectT
from SearchKernel.V2.InspectQueryRequest import InspectQueryRequestT


@dataclass(frozen=True)
class Tensor:
    """A portable dense tensor value for documents and query inputs."""

    type_spec: str
    values: tuple[float, ...]

    @classmethod
    def dense(cls, type_spec: str, values: Sequence[float]) -> "Tensor":
        if not isinstance(type_spec, str) or not type_spec.strip():
            raise ValueError("tensor type spec must be a non-empty string")
        converted = []
        for item in values:
            if isinstance(item, bool) or not isinstance(item, (int, float)):
                raise TypeError("dense tensor cells must be numeric")
            cell = float(item)
            if not math.isfinite(cell):
                raise ValueError("tensor cells must be finite")
            converted.append(cell)
        return cls(type_spec=type_spec, values=tuple(converted))


def value(item: Any) -> ValueT:
    """Convert ordinary Python values into generated V2 ValueT objects."""
    if isinstance(item, ValueT):
        return item
    if item is None:
        return ValueT(kindType=ValueKind.NullValueValue, kind=NullValueValueT(value=NullValue.Null))
    if isinstance(item, bool):
        return ValueT(kindType=ValueKind.BoolValue, kind=BoolValueT(value=item))
    if isinstance(item, int):
        return ValueT(kindType=ValueKind.Int64Value, kind=Int64ValueT(value=item))
    if isinstance(item, float):
        if not math.isfinite(item):
            raise ValueError("non-finite floats are not portable")
        return ValueT(kindType=ValueKind.Float64Value, kind=Float64ValueT(value=item))
    if isinstance(item, str):
        return ValueT(kindType=ValueKind.StringValue, kind=StringValueT(value=item))
    if isinstance(item, bytes):
        return ValueT(kindType=ValueKind.BytesValue, kind=BytesValueT(value=item))
    if isinstance(item, Tensor):
        return ValueT(
            kindType=ValueKind.TensorValue,
            kind=TensorValueT(
                typeSpec=item.type_spec,
                payloadType=TensorPayloadKind.DenseTensorValues,
                payload=DenseTensorValuesT(values=list(item.values)),
            ),
        )
    if isinstance(item, Mapping):
        return ValueT(
            kindType=ValueKind.ValueObject,
            kind=ValueObjectT(fields=[ValueEntryT(key=str(k), value=value(v)) for k, v in sorted(item.items())]),
        )
    if isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
        return ValueT(kindType=ValueKind.ValueList, kind=ValueListT(values=[value(v) for v in item]))
    raise TypeError(f"unsupported value type: {type(item).__name__}")


def _document(identifier: str, fields: Mapping[str, Any]) -> DocumentT:
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("document id must be a non-empty string")
    return DocumentT(
        id=identifier,
        fields=[ValueEntryT(key=str(name), value=value(item)) for name, item in sorted(fields.items())],
    )


def _entries(entries: Sequence[ValueEntryT] | None) -> dict[str, ValueT]:
    decoded: dict[str, ValueT] = {}
    for entry in entries or []:
        if entry is None or entry.key is None:
            raise ValueError("result map entry has no key")
        if entry.key in decoded:
            raise ValueError(f"duplicate result map key: {entry.key}")
        decoded[entry.key] = entry.value
    return decoded


def _require_value_payload(item: ValueT, expected_type: type, label: str) -> Any:
    if not isinstance(item.kind, expected_type):
        raise ValueError(f"{label} value has a missing or mismatched payload")
    return item.kind


def _sequence_values(items: Sequence[Any] | None) -> list[Any]:
    return [] if items is None else list(items)


def _tensor_address(entries: Sequence[Any] | None) -> list[dict[str, Any]]:
    decoded = []
    for entry in entries or []:
        if entry is None or entry.dimension is None:
            raise ValueError("tensor address entry has no dimension")
        decoded.append({"dimension": entry.dimension, "label": entry.label})
    return decoded


def _python_value(item: ValueT | None) -> Any:
    if item is None:
        return None

    if item.kindType == ValueKind.NullValueValue:
        payload = _require_value_payload(item, NullValueValueT, "null")
        if payload.value != NullValue.Null:
            raise ValueError(f"unsupported null value kind: {payload.value}")
        return None
    if item.kindType == ValueKind.BoolValue:
        return _require_value_payload(item, BoolValueT, "bool").value
    if item.kindType == ValueKind.Int64Value:
        return _require_value_payload(item, Int64ValueT, "int64").value
    if item.kindType == ValueKind.UInt64Value:
        return _require_value_payload(item, UInt64ValueT, "uint64").value
    if item.kindType == ValueKind.Float64Value:
        return _require_value_payload(item, Float64ValueT, "float64").value
    if item.kindType == ValueKind.NonFiniteFloatValue:
        kind = _require_value_payload(item, NonFiniteFloatValueT, "non-finite float").kind
        if kind == NonFiniteFloatKind.NaN:
            return float("nan")
        if kind == NonFiniteFloatKind.PositiveInfinity:
            return float("inf")
        if kind == NonFiniteFloatKind.NegativeInfinity:
            return float("-inf")
        raise ValueError(f"unsupported non-finite float kind: {kind}")
    if item.kindType == ValueKind.StringValue:
        payload = _require_value_payload(item, StringValueT, "string")
        if payload.value is None:
            raise ValueError("string value has no string payload")
        return payload.value
    if item.kindType == ValueKind.BytesValue:
        payload = _require_value_payload(item, BytesValueT, "bytes")
        return bytes(payload.value) if payload.value is not None else b""
    if item.kindType == ValueKind.ValueList:
        payload = _require_value_payload(item, ValueListT, "list")
        return [_python_value(child) for child in (payload.values or [])]
    if item.kindType == ValueKind.ValueObject:
        payload = _require_value_payload(item, ValueObjectT, "object")
        return {key: _python_value(value) for key, value in _entries(payload.fields).items()}
    if item.kindType == ValueKind.TensorValue:
        payload = _require_value_payload(item, TensorValueT, "tensor")
        if payload.typeSpec is None:
            raise ValueError("tensor value has no type spec")
        if payload.payloadType == TensorPayloadKind.DenseTensorValues:
            dense = payload.payload
            if not isinstance(dense, DenseTensorValuesT):
                raise ValueError("dense tensor has a missing or mismatched payload")
            return {
                "$searchkernel_tensor": {
                    "type_spec": payload.typeSpec,
                    "dense": _sequence_values(dense.values),
                }
            }
        if payload.payloadType == TensorPayloadKind.SparseTensorCells:
            sparse = payload.payload
            if not isinstance(sparse, SparseTensorCellsT):
                raise ValueError("sparse tensor has a missing or mismatched payload")
            return {
                "$searchkernel_tensor": {
                    "type_spec": payload.typeSpec,
                    "sparse": [
                        {
                            "address": _tensor_address(cell.address),
                            "value": cell.value,
                        }
                        for cell in (sparse.cells or [])
                        if cell is not None
                    ],
                }
            }
        if payload.payloadType == TensorPayloadKind.MixedTensorBlocks:
            mixed = payload.payload
            if not isinstance(mixed, MixedTensorBlocksT):
                raise ValueError("mixed tensor has a missing or mismatched payload")
            return {
                "$searchkernel_tensor": {
                    "type_spec": payload.typeSpec,
                    "mixed": [
                        {
                            "address": _tensor_address(block.address),
                            "values": _sequence_values(block.values),
                        }
                        for block in (mixed.blocks or [])
                        if block is not None
                    ],
                }
            }
        raise ValueError(f"unsupported tensor payload kind: {payload.payloadType}")

    raise ValueError(f"unsupported value kind: {item.kindType}")


class ClientError(RuntimeError):
    """A transport or canonical Search Kernel error."""

    def __init__(self, message: str, *, error: Any = None, cause: Exception | None = None):
        super().__init__(message)
        self.error = error
        self.cause = cause

    @property
    def code(self) -> str:
        names = {
            ErrorCode.Unspecified: "unspecified",
            ErrorCode.InvalidArgument: "invalid_argument",
            ErrorCode.NotFound: "not_found",
            ErrorCode.AlreadyExists: "already_exists",
            ErrorCode.FailedPrecondition: "failed_precondition",
            ErrorCode.Conflict: "conflict",
            ErrorCode.Unsupported: "unsupported",
            ErrorCode.ResourceExhausted: "resource_exhausted",
            ErrorCode.Internal: "internal",
            ErrorCode.Unavailable: "unavailable",
            ErrorCode.DeadlineExceeded: "deadline_exceeded",
        }
        return names.get(getattr(self.error, "code", ErrorCode.Internal), "internal")

    @property
    def message(self) -> str:
        return str(self)

    @property
    def details(self) -> list[Any]:
        return list(getattr(self.error, "details", None) or [])

    @property
    def retryable(self) -> bool:
        return bool(getattr(self.error, "retryable", False))


SearchKernelError = ClientError


def _host_binary() -> Path:
    configured = os.environ.get("SEARCHKERNEL_LOCAL_HOST")
    if configured:
        return Path(configured)
    package_binary = Path(__file__).resolve().parent / "bin" / "searchkerneld"
    if package_binary.is_file():
        return package_binary
    return Path(__file__).resolve().parents[3] / "target" / "debug" / "searchkerneld"


def _signal_managed_process(process: subprocess.Popen[bytes], signum: int) -> None:
    """Signal the daemon and every child it owns in its private process group."""
    if os.name == "posix":
        try:
            os.killpg(process.pid, signum)
        except ProcessLookupError:
            return
    else:
        process.send_signal(signum)


def _cleanup_managed_local_host(
    process: subprocess.Popen[bytes] | None,
    definition_path: Path | None,
    socket_dir: tempfile.TemporaryDirectory,
    force: bool,
) -> None:
    if process is not None:
        if process.poll() is None:
            _signal_managed_process(process, signal.SIGKILL if force else signal.SIGINT)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                _signal_managed_process(process, signal.SIGKILL)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
        if process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
    if definition_path is not None:
        definition_path.unlink(missing_ok=True)
    socket_dir.cleanup()


class _ManagedLocalHost:
    """Own a searchkerneld process and its private Unix-socket resources.

    The daemon starts a new session, making its process group the ownership
    boundary for cleanup. The finalizer is also enabled at interpreter exit so
    callers are not required to explicitly close every managed Index.
    """

    def __init__(self, path: Path, definition: bytes | None):
        self.path = path
        self.definition_path: Path | None = None
        self.process: subprocess.Popen[bytes] | None = None
        self._finalizer: weakref.finalize | None = None
        self.socket_dir = tempfile.TemporaryDirectory(prefix="searchkernel-socket-")
        self.socket_path = Path(self.socket_dir.name) / "searchkerneld.sock"
        self.endpoint = f"unix://{self.socket_path}"
        binary = _host_binary()
        if not binary.is_file():
            self.socket_dir.cleanup()
            raise ClientError(
                f"managed local host binary not found at {binary}; build searchkerneld first"
            )
        if definition is not None:
            temporary = tempfile.NamedTemporaryFile(
                prefix="searchkernel-definition-", suffix=".fb", delete=False
            )
            self.definition_path = Path(temporary.name)
            try:
                temporary.write(definition)
            finally:
                temporary.close()
        environment = os.environ.copy()
        sdk_lib = os.environ.get("SEARCHKERNEL_SDK_LIB")
        if sdk_lib is None:
            candidate = (
                Path(__file__).resolve().parents[3]
                / "core"
                / "native"
                / "sdk"
                / "linux-x86_64"
                / "lib"
            )
            if candidate.is_dir():
                sdk_lib = str(candidate)
        if sdk_lib:
            previous = environment.get("LD_LIBRARY_PATH")
            environment["LD_LIBRARY_PATH"] = f"{sdk_lib}:{previous}" if previous else sdk_lib
        command = [
            str(binary),
            "--index",
            str(path),
            "--listen",
            self.endpoint,
        ]
        if self.definition_path is not None:
            command.extend(["--definition", str(self.definition_path)])
        try:
            self.process = subprocess.Popen(
                command,
                cwd=Path(__file__).resolve().parents[3],
                env=environment,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            self._finalizer = weakref.finalize(
                self,
                _cleanup_managed_local_host,
                self.process,
                self.definition_path,
                self.socket_dir,
                False,
            )
            self._wait_until_ready()
        except Exception:
            self.close(force=True)
            raise

    def _wait_until_ready(self) -> None:
        assert self.process is not None
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise ClientError("managed local host exited during startup")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(self.socket_path))
                    return
                except OSError:
                    time.sleep(0.05)
        raise ClientError("managed local host did not open its gRPC socket")

    def close(self, *, force: bool = False) -> None:
        finalizer = self._finalizer
        if finalizer is not None and not finalizer.alive:
            return
        if finalizer is not None:
            finalizer.detach()
        _cleanup_managed_local_host(self.process, self.definition_path, self.socket_dir, force)
        self.process = None
        self.definition_path = None
        self._finalizer = None


class Client:
    """Synchronous request/response client over a real gRPC channel."""

    def __init__(self, transport: GrpcTransport):
        self.transport = transport

    def execute(self, operation: str, request: Any) -> Any:
        try:
            envelope = unpack_response(self.transport.call(pack_request(operation, request)), operation)
            if envelope.payloadType == ERROR_TYPE:
                error = envelope.payload
                raise ClientError(error.message or "Search Kernel request failed", error=error)
            return envelope.payload
        except ClientError:
            raise
        except (GrpcTransportError, ValueError, TypeError) as exc:
            raise ClientError(str(exc), cause=exc) from exc

    def insert(self, document: DocumentT) -> Any:
        return self.execute("insert", InsertRequestT(document=document))

    def upsert(self, document: DocumentT, condition: str | None = None) -> Any:
        return self.execute("upsert", UpsertRequestT(document=document, condition=condition))

    def delete(self, identifier: str, condition: str | None = None) -> Any:
        return self.execute("delete", DeleteRequestT(id=identifier, condition=condition))

    def patch(self, request: PatchRequestT) -> Any:
        return self.execute("patch", request)

    def update(self, request: UpdateRequestT) -> Any:
        return self.execute("update", request)

    def get(self, identifier: str, projection: Sequence[str] | None = None) -> Any:
        projection_t = FieldProjectionT(fields=list(projection)) if projection is not None else None
        return self.execute("get", GetRequestT(id=identifier, projection=projection_t))

    def search(self, request: Any) -> Any:
        return self.execute("search", request)

    def analyze(self, text: str, language: str | None = None, field: str | None = None) -> Any:
        target = (
            AnalyzeTargetT(AnalyzeTargetKind.AnalyzeFieldTarget, AnalyzeFieldTargetT(field=field))
            if field is not None
            else None
        )
        return self.execute("analyze", AnalyzeRequestT(target=target, text=text, language=language))

    def describe(self) -> Any:
        return self.execute("describe", DescribeRequestT())

    def inspect_query(self, request: Any) -> Any:
        return self.execute("inspect_query", InspectQueryRequestT(request=request))

    def close(self) -> None:
        self.transport.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, _type, _value, _traceback) -> None:
        self.close()


class Index:
    """Low-level V2 client plus the public managed/remote façade."""

    def __init__(self, client: Client, owner: _ManagedLocalHost | None = None, *, ergonomic: bool = False):
        self.client = client
        self._owner = owner
        self._ergonomic = ergonomic

    @classmethod
    def connect(cls, endpoint: str) -> "Index":
        return cls(Client(GrpcTransport(endpoint)), ergonomic=True)

    @classmethod
    def create(
        cls,
        path: str | os.PathLike[str],
        *,
        fields: Mapping[str, Any] | IndexDefinitionT,
        rank_profiles: Sequence[Any] | None = None,
    ) -> "Index":
        owner = _ManagedLocalHost(
            Path(path), pack_object(_index_definition(fields, rank_profiles))
        )
        return cls(Client(GrpcTransport(owner.endpoint)), owner, ergonomic=True)

    @classmethod
    def open(cls, path: str | os.PathLike[str]) -> "Index":
        owner = _ManagedLocalHost(Path(path), None)
        return cls(Client(GrpcTransport(owner.endpoint)), owner, ergonomic=True)

    def insert(self, identifier: str | Mapping[str, Any], fields: Mapping[str, Any] | None = None) -> Any:
        if self._ergonomic and isinstance(identifier, Mapping):
            document = dict(identifier)
            document_id = document.pop("_id", None)
            if not isinstance(document_id, str):
                raise ValueError("document must contain a non-empty string _id")
            return self.client.insert(_document(document_id, document))
        if fields is None or not isinstance(identifier, str):
            raise TypeError("insert expects (id, fields) or an _id document")
        return self.client.insert(_document(identifier, fields))

    def upsert(
        self,
        identifier: str | Mapping[str, Any],
        fields: Mapping[str, Any] | None = None,
        condition: str | None = None,
    ) -> Any:
        if self._ergonomic and isinstance(identifier, Mapping):
            document = dict(identifier)
            document_id = document.pop("_id", None)
            if not isinstance(document_id, str):
                raise ValueError("document must contain a non-empty string _id")
            return self.client.upsert(_document(document_id, document), condition=condition)
        if fields is None or not isinstance(identifier, str):
            raise TypeError("upsert expects (id, fields) or an _id document")
        return self.client.upsert(_document(identifier, fields), condition=condition)

    def delete(self, identifier: str, condition: str | None = None) -> Any:
        result = self.client.delete(identifier, condition=condition)
        return result.deleted if self._ergonomic else result

    def get(self, identifier: str, projection: Sequence[str] | None = None) -> Any:
        result = self.client.get(identifier, projection)
        if not self._ergonomic:
            return result
        if result.document is None:
            return None
        return {
            "_id": result.document.id,
            **{name: _python_value(item) for name, item in _entries(result.document.fields).items()},
        }

    def search(self, request: Any) -> Any:
        if self._ergonomic and isinstance(request, str):
            from .ergonomic import SearchBuilder
            return SearchBuilder(self, request)
        if self._ergonomic:
            raise TypeError("search expects a text query")
        return self.client.search(request)

    def browse(self):
        if not self._ergonomic:
            raise TypeError("browse is only available on a public Index")
        from .ergonomic import SearchBuilder
        return SearchBuilder(self, None)

    def query(self, query: Any):
        if not self._ergonomic:
            raise TypeError("query is only available on a public Index")
        from .ergonomic import SearchBuilder
        return SearchBuilder(self, query)

    def yql(self, source: str):
        if not self._ergonomic:
            raise TypeError("yql is only available on a public Index")
        from .ergonomic import YqlBuilder
        return YqlBuilder(self, source)

    def patch(self, identifier: str):
        if not self._ergonomic:
            raise TypeError("patch is only available on a public Index")
        from .ergonomic import PatchBuilder
        return PatchBuilder(self, identifier)

    def update(self, identifier: str):
        if not self._ergonomic:
            raise TypeError("update is only available on a public Index")
        from .ergonomic import UpdateBuilder
        return UpdateBuilder(self, identifier)

    def analyze(self, text: str, language: str | None = None, field: str | None = None) -> Any:
        return self.client.analyze(text, language, field)

    def describe(self) -> Any:
        return self.client.describe()

    def inspect_query(self, request: Any) -> Any:
        return self.client.inspect_query(request)

    def close(self) -> None:
        self.client.close()
        if self._owner is not None:
            self._owner.close()
            self._owner = None

    def __enter__(self) -> "Index":
        return self

    def __exit__(self, _type, _value, _traceback) -> None:
        self.close()


def _index_definition(
    value_: Mapping[str, Any] | IndexDefinitionT,
    rank_profiles: Sequence[Any] | None = None,
) -> IndexDefinitionT:
    if isinstance(value_, IndexDefinitionT):
        if rank_profiles is not None:
            raise TypeError("rank_profiles cannot accompany an IndexDefinitionT")
        return value_
    fields = []
    for name, specification in value_.items():
        if hasattr(specification, "definition"):
            fields.append(specification.definition(name))
        elif isinstance(specification, FieldDefinitionT):
            fields.append(specification)
        else:
            raise TypeError(f"unsupported field specification for {name!r}")
    profiles = [
        profile.definition() if hasattr(profile, "definition") else profile
        for profile in (rank_profiles or [])
    ]
    return IndexDefinitionT(name="index", fields=fields, rankProfiles=profiles or None)


__all__ = ["Client", "ClientError", "Index", "SearchKernelError", "Tensor", "value"]
