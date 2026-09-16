"""The byte-oriented C5 transport.

This module deliberately knows nothing about the Portable API.  Requests and
responses are opaque buffers at this boundary; the generated client owns their
FlatBuffers representation.
"""

from __future__ import annotations

import ctypes
import threading
from pathlib import Path
from typing import Optional


ABI_VERSION = 2
STATUS_OK = 0


class NativeTransportError(RuntimeError):
    """A library, ABI, handle, or native-call failure."""


class NativeTransport:
    """Own one opaque C5 handle and forward complete byte buffers to it."""

    def __init__(self, library: ctypes.CDLL, handle: int):
        self._library = library
        self._handle: Optional[int] = handle
        self._lock = threading.Lock()

    @classmethod
    def open(cls, library_path: str, open_payload: bytes = b"") -> "NativeTransport":
        payload = bytes(open_payload)
        if b"\0" in payload:
            raise NativeTransportError("open payload contains an embedded NUL")

        try:
            library = ctypes.CDLL(str(Path(library_path)))
        except OSError as error:
            raise NativeTransportError(
                f"could not load native library {library_path!r}: {error}"
            ) from error

        try:
            abi_version = library.sk_abi_version
            abi_version.restype = ctypes.c_uint32
            abi_version.argtypes = []
            version = int(abi_version())
        except (AttributeError, OSError, TypeError) as error:
            raise NativeTransportError("native library has no usable C5 ABI probe") from error
        if version != ABI_VERSION:
            raise NativeTransportError(
                f"incompatible C5 ABI version {version}; expected {ABI_VERSION}"
            )

        try:
            open_fn = library.sk_open
            open_fn.argtypes = [
                ctypes.c_char_p,
                ctypes.POINTER(ctypes.c_uint64),
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
            ]
            open_fn.restype = ctypes.c_uint32
            call_fn = library.sk_call
            call_fn.argtypes = [
                ctypes.c_uint64,
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
                ctypes.POINTER(ctypes.c_size_t),
                ctypes.POINTER(ctypes.c_uint64),
            ]
            call_fn.restype = ctypes.c_uint32
            close_fn = library.sk_close
            close_fn.argtypes = [
                ctypes.c_uint64,
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
            ]
            close_fn.restype = ctypes.c_uint32
            free_fn = library.sk_buffer_free
            free_fn.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64]
            free_fn.restype = None
        except AttributeError as error:
            raise NativeTransportError("native library is missing a C5 entry point") from error

        path = ctypes.create_string_buffer(payload + b"\0")
        handle = ctypes.c_uint64(0)
        error = (ctypes.c_uint8 * 4096)()
        try:
            status = int(
                open_fn(
                    ctypes.cast(path, ctypes.c_char_p),
                    ctypes.byref(handle),
                    error,
                    len(error),
                )
            )
        except OSError as native_error:
            raise NativeTransportError(f"native open failed: {native_error}") from native_error
        if status != STATUS_OK:
            raise NativeTransportError(
                f"native open failed with status {status}: {_error_text(error)}"
            )
        if handle.value == 0:
            raise NativeTransportError("native open returned an invalid handle")
        return cls(library, int(handle.value))

    def call(self, request: bytes) -> bytes:
        request_bytes = bytes(request)
        with self._lock:
            handle = self._handle
        if handle is None:
            raise NativeTransportError("transport is closed")

        request_storage = None
        if request_bytes:
            request_storage = (ctypes.c_uint8 * len(request_bytes)).from_buffer_copy(request_bytes)
            request_ptr = ctypes.cast(request_storage, ctypes.POINTER(ctypes.c_uint8))
        else:
            request_ptr = ctypes.POINTER(ctypes.c_uint8)()

        response_ptr = ctypes.POINTER(ctypes.c_uint8)()
        response_len = ctypes.c_size_t(0)
        allocation_id = ctypes.c_uint64(0)
        try:
            status = int(
                self._library.sk_call(
                    ctypes.c_uint64(handle),
                    request_ptr,
                    len(request_bytes),
                    ctypes.byref(response_ptr),
                    ctypes.byref(response_len),
                    ctypes.byref(allocation_id),
                )
            )
            if status != STATUS_OK:
                raise NativeTransportError(f"native call failed with status {status}")
            if response_len.value and not bool(response_ptr):
                raise NativeTransportError("native call returned a null response buffer")
            result = ctypes.string_at(response_ptr, response_len.value) if response_len.value else b""
            return bytes(result)
        except OSError as native_error:
            raise NativeTransportError(f"native call failed: {native_error}") from native_error
        finally:
            if bool(response_ptr) or allocation_id.value:
                self._library.sk_buffer_free(response_ptr, allocation_id)

    def close(self) -> None:
        with self._lock:
            handle = self._handle
            self._handle = None
        if handle is None:
            return

        error = (ctypes.c_uint8 * 4096)()
        try:
            status = int(self._library.sk_close(handle, error, len(error)))
        except OSError as native_error:
            raise NativeTransportError(f"native close failed: {native_error}") from native_error
        if status != STATUS_OK:
            raise NativeTransportError(
                f"native close failed with status {status}: {_error_text(error)}"
            )

    def __enter__(self) -> "NativeTransport":
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()


def _error_text(buffer) -> str:
    return bytes(buffer).split(b"\0", 1)[0].decode("utf-8", "replace")
