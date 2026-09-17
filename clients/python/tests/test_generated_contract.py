"""Focused adversarial coverage for the generated Python boundary."""

import sys
import threading
import time
import unittest
import gc
import json
from pathlib import Path

sys.path.insert(0, "clients/python")
sys.path.insert(0, "clients/python/tests")

from searchkernel.generated.client import (
    Analyzer,
    ClientError,
    FieldDefinition,
    IndexDefinition,
    ProtocolError,
    SearchField,
    SearchFieldSelection,
    SearchRequest,
    Value,
    _Reader,
    _read_index,
    _read_response,
    _value_from_json,
    _value_to_json,
    encode_index_definition,
    request_from_json,
    request_from_json_text,
)
from searchkernel.transport import NativeTransport


class GeneratedContractTest(unittest.TestCase):
    def test_c7_response_comparison_is_fail_closed(self):
        from c7_adapter import _assert_exact_response

        expected = {
            "protocol_version": 1,
            "response": {"insert": {"id": "fixture"}},
        }
        _assert_exact_response("insert", expected, expected)

        mutated = {
            "protocol_version": 1,
            "response": {"insert": {"id": "wrong"}},
        }
        with self.assertRaises(AssertionError):
            _assert_exact_response("insert", mutated, expected)

    def test_standalone_index_definition_encoder(self):
        expected = IndexDefinition(
            name="products",
            fields=[FieldDefinition(
                name="title",
                kind="text",
                cardinality="single",
                stored=True,
                searchable=True,
                analyzer=Analyzer(name="english", version=2),
            )],
            default_search_fields=SearchFieldSelection(
                fields=[SearchField(field="title", weight=0.0)]
            ),
        )
        encoded = encode_index_definition(expected)
        reader = _Reader(encoded)
        decoded = _read_index(reader, reader.scalar(0, "<I", 0))
        self.assertEqual(decoded, expected)

    def test_integer_json_is_canonical(self):
        for value in (
            {"int64": 1},
            {"int64": "01"},
            {"int64": "+1"},
            {"int64": "-0"},
            {"uint64": "-1"},
        ):
            with self.assertRaises(ValueError):
                _value_from_json(value)
        self.assertEqual(_value_from_json({"int64": "-1"}).value, -1)
        self.assertEqual(_value_from_json({"null": None}).kind, "null")
        self.assertEqual(_value_to_json(Value.null()), {"null": None})
        with self.assertRaises(ValueError):
            _value_from_json({"null": {}})

    def test_unions_and_payload_fields_are_strict(self):
        with self.assertRaises(ValueError):
            request_from_json({"protocol_version": 1, "request": {"get": {}, "search": {}}})
        with self.assertRaises(ValueError):
            request_from_json({"protocol_version": 1, "request": {"search": {"query": "x", "extra": 1}}})
        operation, request = request_from_json(
            {"protocol_version": 1, "request": {"patch": {"id": "a"}}}
        )
        self.assertEqual(operation, "patch")
        self.assertEqual(request.operations, [])
        operation, request = request_from_json(
            {"protocol_version": 1, "request": {"search": {"query": "x"}}}
        )
        self.assertIs(type(request), SearchRequest)
        operation, request = request_from_json(
            {"protocol_version": 1, "request": {"patch": {"id": "a", "operations": [{"none": {}}]}}}
        )
        self.assertEqual(operation, "patch")
        self.assertEqual(request.operations[0].kind, "none")

    def test_search_nested_shapes_and_enums_are_strict(self):
        invalid = [
            {"projection": "title"},
            {"highlighting": "title"},
            {"sort": [{"field": "x", "bogus": 1}]},
            {"facets": [{"field": "x", "bogus": 1}]},
            {"sort": [{"field": "x", "direction": "bogus"}]},
            {"facets": [{"field": "x", "sort": "bogus"}]},
        ]
        for options in invalid:
            with self.subTest(options=options), self.assertRaises(ValueError):
                request_from_json(
                    {"protocol_version": 1, "request": {"search": {"query": "x", **options}}}
                )

        with self.assertRaises(ValueError):
            request_from_json(
                {"protocol_version": 1, "request": {"get": {"id": "x", "projection": "title"}}}
            )

    def test_duplicate_json_fields_are_rejected(self):
        with self.assertRaises(ValueError):
            request_from_json_text(
                '{"protocol_version":1,"request":{"search":{"query":"one","query":"two"}}}'
            )

        operation, request = request_from_json_text(
            '{"protocol_version":1,"request":{"search":{"query":"x"}}}'
        )
        self.assertEqual(operation, "search")
        self.assertEqual(request.query, "x")

    def test_request_corpus_and_c6_negative_shapes(self):
        request_dir = Path("reference/json-conformance/generated/requests")
        for path in sorted(request_dir.glob("*.json")):
            request_from_json(json.loads(path.read_text(encoding="utf-8")))

        invalid = [
            '{"protocol_version":1,"request":{"search":{"query":"x","limt":20}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","query":"y"}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","timeout":NaN}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","limit":null}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","filter":null}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","filter":{}}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","sort":[{"field":"x","direction":"made_up"}]}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","facets":[{"field":"x","extra":1}]}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","search_fields":[{"field":1}]}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","projection":[1]}}}',
            '{"protocol_version":true,"request":{"describe":{}}}',
            '{"protocol_version":1,"request":{"get":{"id":1}}}',
            '{"protocol_version":1,"request":{"patch":{"id":"a","operations":[{"set_field":{"field":"x","value":{"int64":1}}}]}}}',
            '{"protocol_version":1,"request":{"analyze":{"text":1}}}',
            '{"protocol_version":1,"request":{"search":{"query":"x","query_expr":{"term_query":{"field":"x","value":{"string":"y"},"extra":1}}}}}',
        ]
        for raw in invalid:
            with self.assertRaises((TypeError, ValueError)):
                request_from_json_text(raw)

    def test_malformed_response_is_public_client_error(self):
        class Transport:
            def call(self, _request):
                return b"bad"

        from searchkernel.generated.client import Client, GetRequest

        with self.assertRaises(Exception) as raised:
            Client(Transport()).get(GetRequest(id="id"))
        self.assertIn("malformed native response", str(raised.exception))

    def test_response_corpus_and_byte_corruption_are_protocol_safe(self):
        response_dir = Path("core/crates/searchkernel-json/tests/fixtures/responses")
        normal_operations = {
            "analyze", "delete", "describe", "get", "insert", "inspect_query",
            "patch", "search", "upsert",
        }
        for path in sorted(response_dir.glob("*.skrs")):
            operation = "insert" if path.stem.startswith("error-") else path.stem
            self.assertIn(operation, normal_operations)
            payload = path.read_bytes()
            if operation == "insert" and path.stem.startswith("error-"):
                with self.assertRaises(ClientError):
                    _read_response(payload, operation)
            else:
                _read_response(payload, operation)

            # Flip bytes through the envelope header, vtable, and the first
            # payload region. A corrupted discriminator or offset may be
            # rejected, but it must never escape as struct.error, IndexError,
            # or another implementation exception.
            for offset in range(min(len(payload), 256)):
                corrupted = bytearray(payload)
                corrupted[offset] ^= 0x01
                try:
                    _read_response(bytes(corrupted), operation)
                except ClientError:
                    pass
                except ProtocolError:
                    pass
                except Exception as error:
                    self.fail(f"{path.name} corruption at {offset} leaked {type(error).__name__}: {error}")

    def test_close_waits_for_an_admitted_call(self):
        entered = threading.Event()
        release = threading.Event()
        closed = threading.Event()

        class Library:
            def sk_call(self, *_args):
                entered.set()
                release.wait(2)
                return 0

            def sk_close(self, *_args):
                closed.set()
                return 0

            def sk_buffer_free(self, *_args):
                pass

        transport = NativeTransport(Library(), 7)
        call = threading.Thread(target=lambda: transport.call(b"request"))
        call.start()
        self.assertTrue(entered.wait(1))
        closer = threading.Thread(target=transport.close)
        closer.start()
        time.sleep(0.02)
        self.assertFalse(closed.is_set())
        release.set()
        call.join(1)
        closer.join(1)
        self.assertTrue(closed.is_set())

    def test_abandoned_transport_is_finalized(self):
        closed = threading.Event()

        class Library:
            def sk_close(self, *_args):
                closed.set()
                return 0

        transport = NativeTransport(Library(), 9)
        del transport
        gc.collect()
        self.assertTrue(closed.wait(1))


if __name__ == "__main__":
    unittest.main()
