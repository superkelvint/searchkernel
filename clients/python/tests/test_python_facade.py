"""Tests for handwritten Python product ergonomics."""

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import (  # noqa: E402
    DeleteResult,
    DescribeResult,
    DocumentResult,
    FacetResult,
    FacetValue,
    GetResult,
    IndexDefinition,
    FieldDefinition,
    Hit as PublicHit,
    Index,
    SearchResult,
    Text,
    Keyword,
    Float,
    field,
)
from searchkernel.client import (  # noqa: E402
    _GeneratedValue,
    _native_library_name,
    _resolve_native_library,
)
from searchkernel.generated.client import (  # noqa: E402
    TotalHits,
    Value,
)


class FakeTransport:
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


class FakeClient:
    def __init__(self):
        self.calls = []
        self.get_result = GetResult(
            document=DocumentResult(id="1", fields={"price": Value.float64(99.0)})
        )
        self.search_result = SearchResult(
            hits=[HitValue()],
            total_hits=TotalHits(value=1, relation="exact"),
            facets=[
                FacetResult(
                    field="brand",
                    values=[FacetValue(value=Value.string("Acme"), count=1)],
                )
            ],
        )

    def upsert(self, request):
        self.calls.append(("upsert", request))

    def patch(self, request):
        self.calls.append(("patch", request))

    def get(self, request):
        self.calls.append(("get", request))
        return self.get_result

    def delete(self, request):
        self.calls.append(("delete", request))
        return DeleteResult(id=request.id, deleted=True)

    def search(self, request):
        self.calls.append(("search", request))
        return self.search_result

    def describe(self, request):
        self.calls.append(("describe", request))
        return DescribeResult(
            index=IndexDefinition(
                fields=[FieldDefinition(name="price", kind="float64")]
            )
        )


class HitValue:
    id = "1"
    score = 2.0
    fields = {"title": Value.string("Wireless headphones")}
    highlights = []


class FacadeTest(unittest.TestCase):
    def test_field_comparison_and_schema_helpers_use_generated_models(self):
        expression = field("price") <= 100
        self.assertEqual(expression.kind, "comparison")
        self.assertEqual(expression.field, "price")
        self.assertEqual(expression.op, "lte")
        self.assertEqual(expression.value.kind, "int64")
        self.assertEqual(Text(stored=True).definition("title").kind, "text")
        self.assertEqual(Keyword(filterable=True).definition("brand").kind, "keyword")
        self.assertEqual(Float(sortable=True).definition("price").kind, "float64")

    def test_value_conversion_preserves_python_types(self):
        from searchkernel.client import _python_value, _value

        value = _value({"ok": True, "price": 1.5, "raw": b"x", "items": [None]})
        self.assertEqual(value.kind, "object")
        self.assertEqual(_python_value(value), {"ok": True, "price": 1.5, "raw": b"x", "items": [None]})

    def test_crud_and_search_facade_uses_generated_client(self):
        transport = FakeTransport()
        index = Index(transport)
        client = FakeClient()
        index._client = client

        index.put({"_id": "1", "title": "Wireless headphones", "price": 99.0})
        index.update("1", {"price": 89.0})
        self.assertEqual(index.get("1"), {"_id": "1", "price": 99.0})
        self.assertTrue(index.delete("1"))
        results = index.search("wireless", filter=field("price") <= 100, facets=["brand"], limit=10)
        self.assertIsInstance(results.hits[0], PublicHit)
        self.assertEqual(results.hits[0]["_id"], "1")
        self.assertEqual(results.facets["brand"][0], {"value": "Acme", "count": 1})
        self.assertEqual(client.calls[0][0], "upsert")
        self.assertEqual(client.calls[1][0], "patch")
        self.assertEqual(client.calls[-1][0], "search")

        index.close()
        index.close()
        self.assertTrue(transport.closed)

    def test_create_uses_environment_library_override_and_generated_definition(self):
        transport = FakeTransport()
        with patch.dict("os.environ", {"SEARCHKERNEL_NATIVE_LIBRARY": "/tmp/native.so"}), \
             patch("searchkernel.client.Path.is_file", return_value=True), \
             patch("searchkernel.client.NativeTransport.create", return_value=transport) as create:
            index = Index.create("./tmp/products", fields={"title": Text(stored=True)})
        create.assert_called_once()
        args = create.call_args.args
        self.assertEqual(args[0], "/tmp/native.so")
        self.assertEqual(args[1], "./tmp/products")
        self.assertIsInstance(args[2], bytes)
        self.assertIs(index._transport, transport)

    def test_missing_bundled_library_fails_without_target_directory_probe(self):
        with patch.dict("os.environ", {}, clear=True), \
             patch("searchkernel.client.platform.system", return_value="Linux"), \
             patch("searchkernel.client.platform.machine", return_value="x86_64"), \
             patch("searchkernel.client.files") as package_files:
            package_files.return_value.joinpath.return_value.is_file.return_value = False
            with self.assertRaisesRegex(Exception, "not bundled"):
                _resolve_native_library()

    def test_open_describes_schema_for_ergonomic_numeric_filters(self):
        transport = FakeTransport()
        client = FakeClient()
        with patch.dict("os.environ", {"SEARCHKERNEL_NATIVE_LIBRARY": "/tmp/native.so"}), \
             patch("searchkernel.client.Path.is_file", return_value=True), \
             patch("searchkernel.client.NativeTransport.open", return_value=transport), \
             patch("searchkernel.client._GeneratedClient", return_value=client):
            index = Index.open("./tmp/products")

        index.search(filter=field("price") <= 100)
        request = client.calls[-1][1]
        self.assertEqual(request.filter.value.kind, "float64")
        self.assertEqual(client.calls[0][0], "describe")


if __name__ == "__main__":
    unittest.main()
