"""Synchronous Python client for the canonical gRPC/V2 transport."""

from __future__ import annotations

import math
import os
import signal
import socket
import subprocess
import tempfile
import time
from collections.abc import Mapping, Sequence
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
from SearchKernel.V2.FieldDefinition import FieldDefinitionT
from SearchKernel.V2.FieldProjection import FieldProjectionT
from SearchKernel.V2.Float64Value import Float64ValueT
from SearchKernel.V2.GetRequest import GetRequestT
from SearchKernel.V2.IndexDefinition import IndexDefinitionT
from SearchKernel.V2.InsertRequest import InsertRequestT
from SearchKernel.V2.Int64Value import Int64ValueT
from SearchKernel.V2.MatchingConfig import MatchingConfigT
from SearchKernel.V2.MatchingMode import MatchingMode
from SearchKernel.V2.OptionalBool import OptionalBoolT
from SearchKernel.V2.PatchRequest import PatchRequestT
from SearchKernel.V2.StringValue import StringValueT
from SearchKernel.V2.UInt64Value import UInt64ValueT
from SearchKernel.V2.UpsertRequest import UpsertRequestT
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.ValueEntry import ValueEntryT
from SearchKernel.V2.ValueKind import ValueKind
from SearchKernel.V2.ValueList import ValueListT
from SearchKernel.V2.ValueObject import ValueObjectT
from SearchKernel.V2.InspectQueryRequest import InspectQueryRequestT


def value(item: Any) -> ValueT:
    """Convert ordinary Python values into generated V2 ValueT objects."""
    if isinstance(item, ValueT):
        return item
    if item is None:
        return ValueT(kindType=ValueKind.NullValueValue)
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
    if isinstance(item, Mapping):
        return ValueT(
            kindType=ValueKind.ValueObject,
            kind=ValueObjectT(fields=[ValueEntryT(key=str(k), value=value(v)) for k, v in item.items()]),
        )
    if isinstance(item, Sequence) and not isinstance(item, (str, bytes, bytearray)):
        return ValueT(kindType=ValueKind.ValueList, kind=ValueListT(values=[value(v) for v in item]))
    raise TypeError(f"unsupported value type: {type(item).__name__}")


def _document(identifier: str, fields: Mapping[str, Any]) -> DocumentT:
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("document id must be a non-empty string")
    return DocumentT(
        id=identifier,
        fields=[ValueEntryT(key=str(name), value=value(item)) for name, item in fields.items()],
    )


def _entries(entries: Sequence[ValueEntryT] | None) -> dict[str, ValueT]:
    return {
        entry.key: entry.value
        for entry in entries or []
        if entry is not None and entry.key is not None
    }


def _python_value(item: ValueT | None) -> Any:
    if item is None:
        return None
    if item.kindType == ValueKind.ValueList:
        payload = item.kind
        return [_python_value(child) for child in (payload.values if payload else [])]
    if item.kindType == ValueKind.ValueObject:
        payload = item.kind
        return {
            entry.key: _python_value(entry.value)
            for entry in (payload.fields if payload else [])
            if entry is not None
        }
    return getattr(item.kind, "value", None)


class ClientError(RuntimeError):
    """A transport or canonical Search Kernel error."""

    def __init__(self, message: str, *, error: Any = None, cause: Exception | None = None):
        super().__init__(message)
        self.error = error
        self.cause = cause

    @property
    def code(self) -> str:
        names = {
            0: "unspecified", 1: "invalid_argument", 2: "not_found",
            3: "already_exists", 4: "failed_precondition", 5: "conflict",
            6: "unsupported", 7: "resource_exhausted", 8: "internal",
            9: "unavailable", 10: "deadline_exceeded",
        }
        return names.get(getattr(self.error, "code", 8), "internal")

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


class _ManagedLocalHost:
    def __init__(self, path: Path, definition: bytes | None):
        self.path = path
        self.definition_path: Path | None = None
        self.process: subprocess.Popen[bytes] | None = None
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
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
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
                stderr = self.process.stderr.read().decode(errors="replace") if self.process.stderr else ""
                raise ClientError(f"managed local host exited during startup: {stderr}")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(self.socket_path))
                    return
                except OSError:
                    time.sleep(0.05)
        raise ClientError("managed local host did not open its gRPC socket")

    def close(self, *, force: bool = False) -> None:
        process, self.process = self.process, None
        if process is not None and process.poll() is None:
            if force:
                process.kill()
            else:
                process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        if process is not None:
            if process.stdout is not None:
                process.stdout.close()
            if process.stderr is not None:
                process.stderr.close()
        if self.definition_path is not None:
            self.definition_path.unlink(missing_ok=True)
            self.definition_path = None
        self.socket_dir.cleanup()


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
    ) -> "Index":
        owner = _ManagedLocalHost(Path(path), pack_object(_index_definition(fields)))
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


def _index_definition(value_: Mapping[str, Any] | IndexDefinitionT) -> IndexDefinitionT:
    if isinstance(value_, IndexDefinitionT):
        return value_
    fields = []
    for name, specification in value_.items():
        if hasattr(specification, "definition"):
            fields.append(specification.definition(name))
        elif isinstance(specification, FieldDefinitionT):
            fields.append(specification)
        else:
            raise TypeError(f"unsupported field specification for {name!r}")
    return IndexDefinitionT(name="index", fields=fields)


__all__ = ["Client", "ClientError", "Index", "SearchKernelError", "value"]
