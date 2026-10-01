"""Focused tests for the Python gRPC transport policy."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Client, ClientError  # noqa: E402
from searchkernel.grpc_transport import (  # noqa: E402
    DEFAULT_GRPC_TIMEOUT_SECONDS,
    MAX_GRPC_MESSAGE_SIZE,
    GrpcTransport,
)
from searchkernel.protocol import pack_object, pack_request  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.ResponseEnvelope import ResponseEnvelopeT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.ResponsePayload import ResponsePayload  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.CreateIndexResult import CreateIndexResultT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.IndexDefinition import IndexDefinitionT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.OpenIndexResult import OpenIndexResultT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.RequestEnvelope import RequestEnvelope  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.InsertRequest import InsertRequestT  # noqa: E402


class _Stub:
    def __init__(self, _channel):
        self.timeout = None

    def Execute(self, _request, *, timeout):
        self.timeout = timeout
        return SimpleNamespace(payload=b"response")


class _FrameTransport:
    def __init__(self, frames):
        self.frames = list(frames)
        self.closed = False

    def call(self, _payload):
        return self.frames.pop(0)

    def close(self):
        self.closed = True


def _response(payload_type, payload):
    return pack_object(
        ResponseEnvelopeT(1, payload_type, payload),
        file_identifier=b"SKRS",
    )


class GrpcTransportTest(unittest.TestCase):
    def test_ordinary_request_envelope_carries_logical_index_target(self):
        payload = pack_request("insert", InsertRequestT(), target="products")
        self.assertTrue(RequestEnvelope.RequestEnvelopeBufferHasIdentifier(payload, 0))
        request = RequestEnvelope.GetRootAs(payload)
        self.assertEqual(request.Target().Name().decode(), "products")

    def test_domain_invalid_target_is_encoded_for_canonical_server_validation(self):
        payload = pack_request("insert", InsertRequestT(), target="Products")
        request = RequestEnvelope.GetRootAs(payload)
        self.assertEqual(request.Target().Name().decode(), "Products")

    def test_lifecycle_success_requires_index_definition(self):
        malformed_create = _response(
            ResponsePayload.SearchKernel_Wire_CreateIndexResult,
            CreateIndexResultT(),
        )
        malformed_open = _response(
            ResponsePayload.SearchKernel_Wire_OpenIndexResult,
            OpenIndexResultT(),
        )
        client = Client(_FrameTransport([malformed_create, malformed_open]), target="products")

        with self.assertRaisesRegex(ClientError, "CreateIndexResult.index is missing"):
            client.create_index(IndexDefinitionT(name="products"))
        with self.assertRaisesRegex(ClientError, "OpenIndexResult.index is missing"):
            client.open_index()

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
