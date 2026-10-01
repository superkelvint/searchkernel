"""Synchronous Python client for the canonical gRPC/V2 transport."""

from __future__ import annotations

import hashlib
import math
import os
import signal
import stat
import socket
import subprocess
import tempfile
import threading
import time
import weakref
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, BinaryIO

from .grpc_transport import GrpcTransport, GrpcTransportError
from .protocol import ERROR_TYPE, pack_request, unpack_response
from .scalars import check_int64

from SearchKernel.V2.AnalyzeFieldTarget import AnalyzeFieldTargetT
from SearchKernel.V2.AnalyzeRequest import AnalyzeRequestT
from SearchKernel.V2.AnalyzeTarget import AnalyzeTarget as AnalyzeTargetKind
from SearchKernel.V2.BoolValue import BoolValueT
from SearchKernel.V2.BytesValue import BytesValueT
from SearchKernel.V2.DataType import DataTypeT
from SearchKernel.V2.DataTypeKind import DataTypeKind
from SearchKernel.V2.DeleteRequest import DeleteRequestT
from SearchKernel.V2.DeleteIndexRequest import DeleteIndexRequestT
from SearchKernel.V2.DescribeRequest import DescribeRequestT
from SearchKernel.V2.Document import DocumentT
from SearchKernel.V2.ErrorCode import ErrorCode
from SearchKernel.V2.FieldDefinition import FieldDefinitionT
from SearchKernel.V2.FieldProjection import FieldProjectionT
from SearchKernel.V2.Float64Value import Float64ValueT
from SearchKernel.V2.GetRequest import GetRequestT
from SearchKernel.V2.CreateIndexRequest import CreateIndexRequestT
from SearchKernel.V2.OpenIndexRequest import OpenIndexRequestT
from SearchKernel.V2.IndexDefinition import IndexDefinitionT
from SearchKernel.V2.InsertRequest import InsertRequestT
from SearchKernel.V2.Int64Value import Int64ValueT
from SearchKernel.V2.NonFiniteFloatKind import NonFiniteFloatKind
from SearchKernel.V2.NonFiniteFloatValue import NonFiniteFloatValueT
from SearchKernel.V2.NullValue import NullValue
from SearchKernel.V2.NullValueValue import NullValueValueT
from SearchKernel.V2.MatchingConfig import MatchingConfigT
from SearchKernel.V2.MatchingMode import MatchingMode
from SearchKernel.V2.OptionalBool import OptionalBoolT
from SearchKernel.V2.PatchRequest import PatchRequestT
from SearchKernel.V2.StringValue import StringValueT
from SearchKernel.V2.DenseTensorValues import DenseTensorValuesT
from SearchKernel.V2.MixedTensorBlocks import MixedTensorBlocksT
from SearchKernel.V2.SparseTensorCells import SparseTensorCellsT
from SearchKernel.V2.TensorBlock import TensorBlockT
from SearchKernel.V2.TensorCell import TensorCellT
from SearchKernel.V2.TensorAddressEntry import TensorAddressEntryT
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
        return ValueT(
            kindType=ValueKind.NullValueValue,
            kind=NullValueValueT(value=NullValue.Null),
        )
    if isinstance(item, bool):
        return ValueT(kindType=ValueKind.BoolValue, kind=BoolValueT(value=item))
    if isinstance(item, int):
        return ValueT(kindType=ValueKind.Int64Value, kind=Int64ValueT(value=check_int64(item)))
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


def uint64(item: int) -> ValueT:
    """Construct an explicitly tagged UInt64 request value."""
    if isinstance(item, bool) or not isinstance(item, int):
        raise TypeError("uint64 value must be an integer")
    if item < 0 or item > 2**64 - 1:
        raise ValueError("uint64 value is out of range")
    return ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=item))


def _document(identifier: str, fields: Mapping[str, Any]) -> DocumentT:
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("document id must be a non-empty string")
    return DocumentT(
        id=identifier,
        fields=[ValueEntryT(key=str(name), value=value(item)) for name, item in sorted(fields.items())],
    )


def _decode_value_entries(entries: Sequence[ValueEntryT] | None) -> dict[str, Any]:
    result = {}
    for entry in entries or []:
        if entry is None or entry.key is None:
            raise ValueError("malformed ValueEntry")
        if entry.key in result:
            raise ValueError(f"duplicate key: {entry.key!r}")
        if entry.value is None:
            raise ValueError("missing Value payload")
        result[entry.key] = _python_value(entry.value)
    return result


def _value_payload(item: ValueT, name: str, expected_type: type):
    if item.kind is None or not isinstance(item.kind, expected_type):
        raise ValueError(f"mismatched Value union: {name} payload")
    return item.kind


def _tensor_address(
    entries: Sequence[TensorAddressEntryT] | None,
) -> list[dict[str, str]]:
    result = []
    for entry in entries or []:
        if (
            entry is None
            or not isinstance(entry.dimension, str)
            or not isinstance(entry.label, str)
        ):
            raise ValueError("malformed tensor address entry")
        result.append({"dimension": entry.dimension, "label": entry.label})
    return result


def _finite_tensor_cells(values: Sequence[float], label: str) -> list[float]:
    result = []
    for cell in values:
        value_ = float(cell)
        if not math.isfinite(value_):
            raise ValueError(f"{label} tensor cells must be finite")
        result.append(value_)
    return result


def _tensor_value(payload: TensorValueT) -> dict[str, Any]:
    tensor = {"type_spec": payload.typeSpec}
    if payload.payloadType == TensorPayloadKind.DenseTensorValues:
        dense = payload.payload
        if not isinstance(dense, DenseTensorValuesT):
            raise ValueError("mismatched Tensor payload: DenseTensorValues")
        if dense.values is None:
            raise ValueError("malformed dense tensor payload")
        tensor["dense"] = _finite_tensor_cells(dense.values, "dense")
    elif payload.payloadType == TensorPayloadKind.SparseTensorCells:
        sparse = payload.payload
        if not isinstance(sparse, SparseTensorCellsT):
            raise ValueError("mismatched Tensor payload: SparseTensorCells")
        if sparse.cells is None:
            raise ValueError("malformed sparse tensor payload")
        cells = []
        for cell in sparse.cells:
            if not isinstance(cell, TensorCellT):
                raise ValueError("malformed sparse tensor cell")
            cells.append({
                "address": _tensor_address(cell.address),
                "value": _finite_tensor_cells([cell.value], "sparse")[0],
            })
        tensor["sparse"] = cells
    elif payload.payloadType == TensorPayloadKind.MixedTensorBlocks:
        mixed = payload.payload
        if not isinstance(mixed, MixedTensorBlocksT):
            raise ValueError("mismatched Tensor payload: MixedTensorBlocks")
        if mixed.blocks is None:
            raise ValueError("malformed mixed tensor payload")
        blocks = []
        for block in mixed.blocks:
            if not isinstance(block, TensorBlockT):
                raise ValueError("malformed mixed tensor block")
            if block.values is None:
                raise ValueError("malformed mixed tensor block")
            blocks.append(
                {
                    "address": _tensor_address(block.address),
                    "values": _finite_tensor_cells(block.values, "mixed"),
                }
            )
        tensor["mixed"] = blocks
    else:
        raise ValueError("unknown Tensor payload union")
    return {"$searchkernel_tensor": tensor}


def _python_value(item: ValueT | None) -> Any:
    # Union payloads are selected by the FlatBuffers discriminator. Do not infer
    # a portable Value kind from the decoded Python shape: [1, 2] is a list,
    # even when its elements could also be represented as bytes.
    if item is None:
        return None
    if item.kindType == ValueKind.NullValueValue:
        payload = _value_payload(item, "NullValueValue", NullValueValueT)
        if payload.value != NullValue.Null:
            raise ValueError(f"unknown NullValue kind: {payload.value}")
        return None
    if item.kindType == ValueKind.BoolValue:
        return _value_payload(item, "BoolValue", BoolValueT).value
    if item.kindType == ValueKind.Int64Value:
        return _value_payload(item, "Int64Value", Int64ValueT).value
    if item.kindType == ValueKind.UInt64Value:
        return _value_payload(item, "UInt64Value", UInt64ValueT).value
    if item.kindType == ValueKind.Float64Value:
        return _value_payload(item, "Float64Value", Float64ValueT).value
    if item.kindType == ValueKind.NonFiniteFloatValue:
        kind = _value_payload(item, "NonFiniteFloatValue", NonFiniteFloatValueT).kind
        if kind == NonFiniteFloatKind.NaN:
            return math.nan
        if kind == NonFiniteFloatKind.PositiveInfinity:
            return math.inf
        if kind == NonFiniteFloatKind.NegativeInfinity:
            return -math.inf
        raise ValueError("unknown NonFiniteFloat kind")
    if item.kindType == ValueKind.StringValue:
        string = _value_payload(item, "StringValue", StringValueT).value
        if string is None:
            raise ValueError("missing StringValue payload")
        return string
    if item.kindType == ValueKind.BytesValue:
        bytes_value = _value_payload(item, "BytesValue", BytesValueT).value
        if bytes_value is None:
            raise ValueError("missing BytesValue payload")
        return bytes(bytes_value)
    if item.kindType == ValueKind.ValueList:
        payload = _value_payload(item, "ValueList", ValueListT)
        if payload.values is None:
            raise ValueError("malformed ValueList payload")
        return [_python_value(child) for child in payload.values]
    if item.kindType == ValueKind.ValueObject:
        payload = _value_payload(item, "ValueObject", ValueObjectT)
        return _decode_value_entries(payload.fields)
    if item.kindType == ValueKind.TensorValue:
        return _tensor_value(_value_payload(item, "TensorValue", TensorValueT))
    raise ValueError(f"unknown Value union kind: {item.kindType}")


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

_MANAGED_HOST_OUTPUT_TAIL_BYTES = 16 * 1024
_MANAGED_HOST_STARTUP_TIMEOUT_SECONDS = 60.0
_MANAGED_HOST_POOL_LOCK = threading.Lock()
_MANAGED_HOST_POOL: dict[str, "_ManagedLocalHost"] = {}


_MANAGED_DATA_ROOT_NAME = ".searchkernel-managed"
_MANAGED_MARKER_NAME = ".searchkernel-managed-target"
_MANAGED_ROOT_MARKER_NAME = ".searchkernel-data-root"
_MANAGED_ROOT_MARKER_VALUE = "searchkernel-managed-v1"
_LEGACY_HOST_ENV_KEYS = (
    "SEARCHKERNEL_INDEX_PATH",
    "SEARCHKERNEL_INDEX_DEFINITION",
    "SEARCHKERNEL_INDEX_DEFINITION_JSON",
)


def _managed_location(path: Path) -> tuple[Path, Path, str]:
    # Keep the caller's lexical path identity stable even after a legacy index
    # is migrated out of that directory. Never realpath through a marker or
    # symlink when deriving the logical target.
    canonical = Path(os.path.abspath(os.path.expanduser(os.fspath(path))))
    target = "local-" + hashlib.sha256(str(canonical).encode("utf-8")).hexdigest()[:32]
    return canonical, canonical.parent / _MANAGED_DATA_ROOT_NAME, target


def _managed_marker(path: Path) -> Path:
    return path / _MANAGED_MARKER_NAME


def _managed_root_error(data_root: Path) -> ClientError:
    return ClientError(f"managed-local data root is not owned by Search Kernel: {data_root}")


def _validate_managed_data_root(
    data_root: Path, *, allow_inflight_publication: bool = False
) -> bool:
    marker = data_root / _MANAGED_ROOT_MARKER_NAME
    try:
        root_stat = data_root.lstat()
        marker_stat = marker.lstat()
    except OSError as exc:
        raise _managed_root_error(data_root) from exc
    if stat.S_ISLNK(root_stat.st_mode) or not stat.S_ISDIR(root_stat.st_mode):
        raise _managed_root_error(data_root)
    if stat.S_ISLNK(marker_stat.st_mode) or not stat.S_ISREG(marker_stat.st_mode):
        raise _managed_root_error(data_root)
    try:
        marker_value = marker.read_text(encoding="utf-8")
    except OSError as exc:
        raise _managed_root_error(data_root) from exc
    if marker_value in (
        _MANAGED_ROOT_MARKER_VALUE,
        _MANAGED_ROOT_MARKER_VALUE + "\n",
        _MANAGED_ROOT_MARKER_VALUE + "\r\n",
    ):
        return True
    # Exclusive creation publishes the marker inode before Python flushes the
    # small buffered payload. A same-process/process contender may therefore
    # observe an empty regular file for a short interval. Only that exact empty
    # state is retryable; any non-empty mismatch still fails closed.
    if allow_inflight_publication and marker_value == "":
        return False
    raise _managed_root_error(data_root)


def _prepare_managed_data_root(data_root: Path) -> None:
    data_root.parent.mkdir(parents=True, exist_ok=True)
    marker = data_root / _MANAGED_ROOT_MARKER_NAME
    created = False
    try:
        data_root.mkdir()
        created = True
    except FileExistsError:
        pass

    if created:
        temporary_marker = None
        published = False
        try:
            descriptor, temporary_marker = tempfile.mkstemp(
                prefix=f".{marker.name}.", dir=data_root
            )
            with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
                stream.write(_MANAGED_ROOT_MARKER_VALUE + "\n")
            # Publish complete marker contents atomically without replacing a
            # path that another process may have created in the meantime.
            os.link(temporary_marker, marker)
            published = True
            os.unlink(temporary_marker)
            temporary_marker = None
            _validate_managed_data_root(data_root)
            return
        except Exception:
            try:
                if published:
                    marker.unlink(missing_ok=True)
                if temporary_marker is not None:
                    os.unlink(temporary_marker)
                data_root.rmdir()
            except OSError:
                pass
            raise

    # Another process may have created the final directory and still be
    # publishing its marker. Wait only for a missing marker; wrong types or
    # marker contents fail closed immediately. Using mkdir rather than rename
    # guarantees we never replace a caller-created empty directory.
    deadline = time.monotonic() + 2.0
    while True:
        try:
            root_stat = data_root.lstat()
        except OSError as exc:
            raise _managed_root_error(data_root) from exc
        if stat.S_ISLNK(root_stat.st_mode) or not stat.S_ISDIR(root_stat.st_mode):
            raise _managed_root_error(data_root)
        try:
            marker.lstat()
        except FileNotFoundError:
            if time.monotonic() >= deadline:
                raise _managed_root_error(data_root)
            time.sleep(0.025)
            continue
        if _validate_managed_data_root(
            data_root, allow_inflight_publication=True
        ):
            return
        if time.monotonic() >= deadline:
            raise _managed_root_error(data_root)
        time.sleep(0.025)


def _managed_target_marker_matches(
    path: Path, target: str, *, allow_inflight_publication: bool = False
) -> bool:
    marker = _managed_marker(path)
    try:
        marker_stat = marker.lstat()
    except FileNotFoundError:
        return False
    except OSError as exc:
        raise ClientError(f"cannot inspect managed-local marker: {marker}") from exc
    if stat.S_ISLNK(marker_stat.st_mode) or not stat.S_ISREG(marker_stat.st_mode):
        raise ClientError(f"unsafe managed-local marker: {marker}")
    try:
        marker_value = marker.read_text(encoding="utf-8")
    except OSError as exc:
        raise ClientError(f"cannot read managed-local marker: {marker}") from exc
    if marker_value.strip() == target:
        return True
    if allow_inflight_publication and marker_value == "":
        return False
    raise ClientError(f"managed-local marker target mismatch at {marker}")


def _write_managed_marker(path: Path, target: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        path.mkdir()
    except FileExistsError:
        try:
            path_stat = path.lstat()
        except OSError as exc:
            raise ClientError(f"cannot inspect managed-local identity path: {path}") from exc
        if stat.S_ISLNK(path_stat.st_mode) or not stat.S_ISDIR(path_stat.st_mode):
            raise ClientError(f"managed-local identity path is not a real directory: {path}")

    marker = _managed_marker(path)
    created = False
    try:
        with marker.open("x", encoding="utf-8") as stream:
            stream.write(target + "\n")
        created = True
    except FileExistsError:
        pass
    if created:
        _managed_target_marker_matches(path, target)
        return

    deadline = time.monotonic() + 2.0
    while True:
        if _managed_target_marker_matches(
            path, target, allow_inflight_publication=True
        ):
            return
        if time.monotonic() >= deadline:
            raise ClientError(f"managed-local marker target mismatch at {marker}")
        time.sleep(0.025)


def _reserve_managed_path(path: Path, target: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        path.mkdir()
    except FileExistsError as exc:
        raise ClientError(f"managed-local index path already exists: {path}") from exc
    try:
        _managed_marker(path).write_text(target + "\n", encoding="utf-8")
    except Exception:
        try:
            path.rmdir()
        except OSError:
            pass
        raise


def _cleanup_managed_reservation(path: Path, target: str) -> None:
    marker = _managed_marker(path)
    try:
        if marker.is_file() and marker.read_text(encoding="utf-8").strip() == target:
            marker.unlink()
            path.rmdir()
    except OSError:
        pass


def _managed_host_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for key in _LEGACY_HOST_ENV_KEYS:
        environment.pop(key, None)
    sdk_lib = os.environ.get("SEARCHKERNEL_SDK_LIB")
    if sdk_lib:
        previous = environment.get("LD_LIBRARY_PATH")
        environment["LD_LIBRARY_PATH"] = f"{sdk_lib}:{previous}" if previous else sdk_lib
    return environment


def _migrate_legacy_managed_path(path: Path, data_root: Path, target: str) -> None:
    destination = data_root / target
    try:
        source_stat = path.lstat()
    except FileNotFoundError:
        source_stat = None
    if source_stat is not None and (
        stat.S_ISLNK(source_stat.st_mode) or not stat.S_ISDIR(source_stat.st_mode)
    ):
        raise ClientError(f"legacy managed-local index must be a real directory: {path}")

    marker_matches = False
    if source_stat is not None:
        marker_matches = _managed_target_marker_matches(path, target)

    try:
        destination_stat = destination.lstat()
    except FileNotFoundError:
        destination_stat = None
    if destination_stat is not None:
        if stat.S_ISLNK(destination_stat.st_mode) or not stat.S_ISDIR(destination_stat.st_mode):
            raise ClientError(f"unsafe managed-local destination: {destination}")
        if source_stat is None or marker_matches:
            return
        raise ClientError(
            f"managed-local destination already exists while legacy source remains: {destination}"
        )
    if marker_matches:
        return
    if source_stat is None:
        return

    binary = _host_binary()
    if not binary.is_file():
        raise ClientError(
            f"managed local host binary not found at {binary}; build searchkerneld first"
        )
    completed = subprocess.run(
        [
            str(binary),
            "--migrate-index",
            str(path),
            "--data-root",
            str(data_root),
            "--target",
            target,
        ],
        cwd=Path(__file__).resolve().parents[3],
        env=_managed_host_environment(),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        # An equivalent concurrent migrator may have completed while this
        # process was waiting on the lifecycle lock. Converge on that result.
        try:
            destination_stat = destination.lstat()
        except FileNotFoundError:
            destination_stat = None
        try:
            source_stat = path.lstat()
        except FileNotFoundError:
            source_stat = None
        if (
            destination_stat is not None
            and stat.S_ISDIR(destination_stat.st_mode)
            and not stat.S_ISLNK(destination_stat.st_mode)
            and source_stat is None
        ):
            _write_managed_marker(path, target)
            return
        output = completed.stdout.decode("utf-8", errors="replace").strip()
        suffix = f"\nsearchkerneld output:\n{output}" if output else ""
        raise ClientError(
            f"managed-local legacy index migration failed for {path}{suffix}"
        )
    _write_managed_marker(path, target)


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


class _ManagedHostOutputCapture:
    def __init__(self, stream: BinaryIO):
        self._stream = stream
        self._tail = bytearray()
        self._truncated = False
        self._lock = threading.Lock()
        self._thread = threading.Thread(
            target=self._drain,
            name="searchkernel-daemon-output",
            daemon=True,
        )
        self._thread.start()

    def _drain(self) -> None:
        try:
            while chunk := self._stream.read(8192):
                with self._lock:
                    self._tail.extend(chunk)
                    overflow = len(self._tail) - _MANAGED_HOST_OUTPUT_TAIL_BYTES
                    if overflow > 0:
                        del self._tail[:overflow]
                        self._truncated = True
        except (OSError, ValueError):
            pass
        finally:
            try:
                self._stream.close()
            except OSError:
                pass

    def tail(self, *, wait_for_eof: bool) -> str:
        if wait_for_eof:
            self._thread.join(timeout=1)
        with self._lock:
            data = bytes(self._tail)
            truncated = self._truncated
        text = data.decode("utf-8", errors="replace").rstrip()
        if not text:
            return ""
        if truncated:
            return (
                f"[last {_MANAGED_HOST_OUTPUT_TAIL_BYTES} bytes of searchkerneld output]\n"
                f"{text}"
            )
        return text

    def close(self) -> None:
        # The drain thread owns the pipe. Closing a buffered reader from another
        # thread while read() is blocked can itself block on the reader lock.
        # Process shutdown produces EOF, so only wait here and let _drain close it.
        self._thread.join(timeout=1)


def _cleanup_managed_local_host(
    process: subprocess.Popen[bytes] | None,
    socket_dir: tempfile.TemporaryDirectory,
    output_capture: _ManagedHostOutputCapture | None,
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
        if output_capture is None and process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
    if output_capture is not None:
        output_capture.close()
    socket_dir.cleanup()


class _ManagedLocalHost:
    """Own a searchkerneld process and its private Unix-socket resources.

    The daemon starts a new session, making its process group the ownership
    boundary for cleanup. The finalizer is also enabled at interpreter exit so
    callers are not required to explicitly close every managed Index.
    """

    def __init__(self, path: Path):
        self.path = path
        self._pool_key: str | None = None
        self._lease_count = 0
        self.process: subprocess.Popen[bytes] | None = None
        self._output_capture: _ManagedHostOutputCapture | None = None
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
        environment = _managed_host_environment()
        command = [
            str(binary),
            "--data-root",
            str(path),
            "--listen",
            self.endpoint,
        ]
        try:
            self.process = subprocess.Popen(
                command,
                cwd=Path(__file__).resolve().parents[3],
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            if self.process.stdout is None:
                raise ClientError("managed local host output capture was not created")
            self._output_capture = _ManagedHostOutputCapture(self.process.stdout)
            self._finalizer = weakref.finalize(
                self,
                _cleanup_managed_local_host,
                self.process,
                self.socket_dir,
                self._output_capture,
                False,
            )
            self._wait_until_ready()
        except Exception:
            self.close(force=True)
            raise

    @classmethod
    def acquire(cls, data_root: Path) -> "_ManagedLocalHost":
        key = str(data_root.expanduser().resolve(strict=False))
        with _MANAGED_HOST_POOL_LOCK:
            existing = _MANAGED_HOST_POOL.get(key)
            if (
                existing is not None
                and existing.process is not None
                and existing.process.poll() is None
            ):
                existing._lease_count += 1
                return existing
            host = cls(Path(key))
            host._pool_key = key
            host._lease_count = 1
            _MANAGED_HOST_POOL[key] = host
            return host

    def _wait_until_ready(self) -> None:
        assert self.process is not None
        deadline = time.monotonic() + _MANAGED_HOST_STARTUP_TIMEOUT_SECONDS
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = (
                    self._output_capture.tail(wait_for_eof=True)
                    if self._output_capture is not None
                    else ""
                )
                message = (
                    "managed local host exited during startup "
                    f"(exit code {self.process.returncode})"
                )
                if output:
                    message = f"{message}\nsearchkerneld output:\n{output}"
                raise ClientError(message)
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(self.socket_path))
                    return
                except OSError:
                    time.sleep(0.05)
        output = (
            self._output_capture.tail(wait_for_eof=False)
            if self._output_capture is not None
            else ""
        )
        message = "managed local host did not open its gRPC socket"
        if output:
            message = f"{message}\nsearchkerneld output:\n{output}"
        raise ClientError(message)

    def close(self, *, force: bool = False) -> None:
        pool_key = self._pool_key
        if pool_key is not None:
            with _MANAGED_HOST_POOL_LOCK:
                if _MANAGED_HOST_POOL.get(pool_key) is self:
                    if not force and self._lease_count > 1:
                        self._lease_count -= 1
                        return
                    _MANAGED_HOST_POOL.pop(pool_key, None)
                self._lease_count = 0
                self._pool_key = None

        finalizer = self._finalizer
        if finalizer is not None and not finalizer.alive:
            return
        if finalizer is not None:
            finalizer.detach()
        _cleanup_managed_local_host(
            self.process,
            self.socket_dir,
            self._output_capture,
            force,
        )
        self.process = None
        self._output_capture = None
        self._finalizer = None


class Client:
    """Synchronous request/response client over a real gRPC channel."""

    def __init__(self, transport: GrpcTransport, *, target: str | None = None):
        self.transport = transport
        self.target = target
        self.index_generation: str | None = None

    def _execute_envelope(self, operation: str, request: Any) -> Any:
        try:
            expected_generation = (
                None
                if operation in {"create_index", "open_index", "delete_index"}
                else self.index_generation
            )
            envelope = unpack_response(
                self.transport.call(
                    pack_request(
                        operation,
                        request,
                        target=self.target,
                        expected_index_generation=expected_generation,
                    )
                ),
                operation,
            )
            if envelope.payloadType == ERROR_TYPE:
                error = envelope.payload
                raise ClientError(error.message or "Search Kernel request failed", error=error)
            return envelope
        except ClientError:
            raise
        except (GrpcTransportError, ValueError, TypeError) as exc:
            raise ClientError(str(exc), cause=exc) from exc

    def execute(self, operation: str, request: Any) -> Any:
        return self._execute_envelope(operation, request).payload

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

    def create_index(self, definition: IndexDefinitionT) -> Any:
        envelope = self._execute_envelope(
            "create_index", CreateIndexRequestT(definition=definition)
        )
        result = envelope.payload
        if getattr(result, "index", None) is None:
            raise ClientError("CreateIndexResult.index is missing")
        generation = getattr(envelope, "indexGeneration", None)
        if not isinstance(generation, str) or not generation:
            raise ClientError("CreateIndexResult is missing canonical index generation")
        self.index_generation = generation
        return result

    def open_index(self) -> Any:
        envelope = self._execute_envelope("open_index", OpenIndexRequestT())
        result = envelope.payload
        if getattr(result, "index", None) is None:
            raise ClientError("OpenIndexResult.index is missing")
        generation = getattr(envelope, "indexGeneration", None)
        if not isinstance(generation, str) or not generation:
            raise ClientError("OpenIndexResult is missing canonical index generation")
        self.index_generation = generation
        return result

    def delete_index(self) -> Any:
        return self.execute("delete_index", DeleteIndexRequestT())

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
    def connect(cls, endpoint: str, target: str | None = None) -> "Index":
        return cls(Client(GrpcTransport(endpoint), target=target), ergonomic=True)

    @classmethod
    def create(
        cls,
        path: str | os.PathLike[str],
        *,
        fields: Mapping[str, Any] | IndexDefinitionT,
        rank_profiles: Sequence[Any] | None = None,
        target: str | None = None,
    ) -> "Index":
        if target is not None:
            client = Client(GrpcTransport(os.fspath(path)), target=target)
            try:
                client.create_index(_index_definition(fields, rank_profiles))
            except Exception:
                client.close()
                raise
            return cls(client, ergonomic=True)
        canonical_path, data_root, target = _managed_location(Path(path))
        _prepare_managed_data_root(data_root)
        _reserve_managed_path(canonical_path, target)
        owner = None
        client = None
        try:
            owner = _ManagedLocalHost.acquire(data_root)
            client = Client(GrpcTransport(owner.endpoint), target=target)
            client.create_index(_index_definition(fields, rank_profiles))
        except Exception:
            if client is not None:
                client.close()
            if owner is not None:
                owner.close()
            _cleanup_managed_reservation(canonical_path, target)
            raise
        return cls(client, owner, ergonomic=True)

    @classmethod
    def open(cls, path: str | os.PathLike[str], *, target: str | None = None) -> "Index":
        if target is not None:
            client = Client(GrpcTransport(os.fspath(path)), target=target)
            try:
                client.open_index()
            except Exception:
                client.close()
                raise
            return cls(client, ergonomic=True)
        canonical_path, data_root, target = _managed_location(Path(path))
        _prepare_managed_data_root(data_root)
        _migrate_legacy_managed_path(canonical_path, data_root, target)
        if (data_root / target).is_dir() and not canonical_path.exists():
            _write_managed_marker(canonical_path, target)
        owner = _ManagedLocalHost.acquire(data_root)
        client = Client(GrpcTransport(owner.endpoint), target=target)
        try:
            client.open_index()
        except Exception:
            client.close()
            owner.close()
            raise
        return cls(client, owner, ergonomic=True)

    @classmethod
    def delete_index(
        cls,
        endpoint_or_path: str | os.PathLike[str],
        *,
        target: str | None = None,
    ) -> None:
        """Delete a logical target, resolving managed-local path identity when needed."""
        if target is not None:
            client = Client(GrpcTransport(os.fspath(endpoint_or_path)), target=target)
            try:
                client.delete_index()
            finally:
                client.close()
            return

        canonical_path, data_root, local_target = _managed_location(Path(endpoint_or_path))
        _prepare_managed_data_root(data_root)
        _migrate_legacy_managed_path(canonical_path, data_root, local_target)
        if (data_root / local_target).is_dir() and not canonical_path.exists():
            _write_managed_marker(canonical_path, local_target)

        owner = _ManagedLocalHost.acquire(data_root)
        client = Client(GrpcTransport(owner.endpoint), target=local_target)
        deleted = False
        try:
            client.delete_index()
            deleted = True
        finally:
            try:
                client.close()
            finally:
                try:
                    owner.close()
                finally:
                    if deleted:
                        _cleanup_managed_reservation(canonical_path, local_target)

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
            **_decode_value_entries(result.document.fields),
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


__all__ = ["Client", "ClientError", "Index", "SearchKernelError", "Tensor", "uint64", "value"]
