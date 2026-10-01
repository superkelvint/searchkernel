"""Synchronous gRPC transport for canonical Search Kernel frames."""

from __future__ import annotations

from typing import Optional

import grpc

from .rpc import searchkernel_pb2, searchkernel_pb2_grpc


MAX_GRPC_MESSAGE_SIZE = 256 * 1024 * 1024
# Ensure a wedged daemon cannot leave a synchronous client call blocked
# forever.
DEFAULT_GRPC_TIMEOUT_SECONDS = 60.0


class GrpcTransportError(RuntimeError):
    """A gRPC channel or RPC failure."""


class GrpcTransport:
    """Forward complete canonical FlatBuffer frames over one gRPC channel."""

    def __init__(self, target: str, *, channel: Optional[grpc.Channel] = None):
        self._owned_channel = channel is None
        options = [
            ("grpc.max_receive_message_length", MAX_GRPC_MESSAGE_SIZE),
            ("grpc.max_send_message_length", MAX_GRPC_MESSAGE_SIZE),
        ]
        if target.startswith("unix:"):
            options.append(("grpc.default_authority", "localhost"))
        self._channel = channel or grpc.insecure_channel(target, options=options)
        self._stub = searchkernel_pb2_grpc.SearchKernelStub(self._channel)
        self._closed = False

    def call(self, payload: bytes) -> bytes:
        if self._closed:
            raise GrpcTransportError("gRPC transport is closed")
        try:
            response = self._stub.Execute(
                searchkernel_pb2.FlatBufferFrame(payload=bytes(payload)),
                timeout=DEFAULT_GRPC_TIMEOUT_SECONDS,
            )
        except grpc.RpcError as error:
            raise GrpcTransportError(
                f"Search Kernel Execute failed ({error.code().name}): {error.details()}"
            ) from error
        return bytes(response.payload)

    def close(self) -> None:
        if not self._closed:
            self._closed = True
            if self._owned_channel:
                self._channel.close()

    def __enter__(self) -> "GrpcTransport":
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()
