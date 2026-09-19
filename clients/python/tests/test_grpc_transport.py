"""Focused tests for the Python gRPC transport policy."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel.grpc_transport import (  # noqa: E402
    DEFAULT_GRPC_TIMEOUT_SECONDS,
    MAX_GRPC_MESSAGE_SIZE,
    GrpcTransport,
)


class _Stub:
    def __init__(self, _channel):
        self.timeout = None

    def Execute(self, _request, *, timeout):
        self.timeout = timeout
        return SimpleNamespace(payload=b"response")


class GrpcTransportTest(unittest.TestCase):
    def test_channel_uses_symmetric_large_message_limits(self):
        channel = object()
        stub = _Stub(channel)
        with (
            patch("searchkernel.grpc_transport.grpc.insecure_channel", return_value=channel) as insecure_channel,
            patch("searchkernel.grpc_transport.searchkernel_pb2_grpc.SearchKernelStub", return_value=stub),
        ):
            GrpcTransport("127.0.0.1:1234")

        insecure_channel.assert_called_once_with(
            "127.0.0.1:1234",
            options=[
                ("grpc.max_receive_message_length", MAX_GRPC_MESSAGE_SIZE),
                ("grpc.max_send_message_length", MAX_GRPC_MESSAGE_SIZE),
            ],
        )

    def test_execute_has_a_finite_transport_deadline(self):
        stub = _Stub(object())
        with patch(
            "searchkernel.grpc_transport.searchkernel_pb2_grpc.SearchKernelStub",
            return_value=stub,
        ):
            transport = GrpcTransport("unused", channel=object())
            self.assertEqual(transport.call(b"request"), b"response")

        self.assertEqual(stub.timeout, DEFAULT_GRPC_TIMEOUT_SECONDS)
        self.assertGreater(DEFAULT_GRPC_TIMEOUT_SECONDS, 0)


if __name__ == "__main__":
    unittest.main()
