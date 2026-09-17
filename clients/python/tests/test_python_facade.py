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
    BrowseBuilder,
    Hit as PublicHit,
    Index,
    SearchResult,
    SearchResults,
    SearchBuilder,
    PatchBuilder,
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

    def insert(self, request):
        self.calls.append(("insert", request))

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

    def test_search_builder_accumulates_state_until_execute(self):
        transport = FakeTransport()
        index = Index(transport)
        client = FakeClient()
        index._client = client

        builder = index.search("shoes")
        self.assertIsInstance(builder, SearchBuilder)
        self.assertIs(builder.filter(field("price") <= 100), builder)
        self.assertIs(builder.sort("rating", "desc"), builder)
        self.assertIs(builder.sort("price", "asc"), builder)
        self.assertIs(builder.facet("brand"), builder)
        self.assertIs(builder.facet("category", limit=10, sort="count_desc"), builder)
        self.assertIs(builder.offset(2), builder)
        self.assertIs(builder.limit(2), builder)
        self.assertEqual(client.calls, [])

        results = builder.execute()
        self.assertIsInstance(results, SearchResults)
        request = client.calls[-1][1]
        self.assertEqual(request.query, "shoes")
        self.assertEqual(request.filter.field, "price")
        self.assertEqual(
            [(item.field, item.direction) for item in request.sort],
            [("rating", "desc"), ("price", "asc")],
        )
        self.assertEqual(
            [(item.field, item.limit, item.sort) for item in request.facets],
            [("brand", None, None), ("category", 10, "count_desc")],
        )
        self.assertEqual((request.offset, request.limit), (2, 2))

    def test_search_builder_validation_and_page_numbering(self):
        index = Index(FakeTransport())

        with self.assertRaises(ValueError):
            index.search("x").sort("price", "sideways")
        with self.assertRaises(TypeError):
            index.search("x").filter("price <= 100")
        with self.assertRaises(ValueError):
            index.search("x").offset(-1)
        with self.assertRaises(ValueError):
            index.search("x").limit(-1)
        with self.assertRaises(ValueError):
            index.search("x").page(0, 20)
        with self.assertRaises(ValueError):
            index.search("x").page(1, 0)

        for page_number, expected_offset in ((1, 0), (2, 20), (3, 40)):
            builder = index.search("x").page(page_number, 20)
            self.assertEqual((builder._offset, builder._limit), (expected_offset, 20))

        conflict_cases = (
            lambda: index.search("x").page(3, 20).offset(40),
            lambda: index.search("x").offset(40).page(3, 20),
            lambda: index.search("x").page(3, 20).limit(20),
            lambda: index.search("x").limit(20).page(3, 20),
        )
        for case in conflict_cases:
            with self.assertRaises((TypeError, ValueError)):
                case()

    def test_browse_builder_uses_match_all_request(self):
        transport = FakeTransport()
        index = Index(transport)
        client = FakeClient()
        index._client = client

        builder = index.browse()
        self.assertIsInstance(builder, BrowseBuilder)
        builder.filter(field("in_stock") == True).sort("price", "asc").execute()

        request = client.calls[-1][1]
        self.assertIsNone(request.query)
        self.assertEqual(request.filter.field, "in_stock")
        self.assertEqual(request.sort[0].direction, "asc")

    def test_patch_builder_preserves_order_and_explicit_unset(self):
        transport = FakeTransport()
        index = Index(transport)
        client = FakeClient()
        index._client = client

        builder = index.patch("1")
        self.assertIsInstance(builder, PatchBuilder)
        self.assertIs(builder.set("price", 79.0), builder)
        self.assertIs(builder.unset("rating"), builder)
        builder.execute()

        request = client.calls[-1][1]
        self.assertEqual(request.id, "1")
        self.assertEqual(
            [(operation.kind, operation.field, operation.value and operation.value.value)
             for operation in request.operations],
            [("set", "price", 79.0), ("unset", "rating", None)],
        )

    def test_upsert_put_and_update_use_direct_generated_operations(self):
        transport = FakeTransport()
        index = Index(transport)
        client = FakeClient()
        index._client = client

        document = {"_id": "1", "price": 99.0}
        index.insert(document)
        index.upsert(document)
        index.put(document)
        index.update("1", {"price": 89.0})

        self.assertEqual([name for name, _ in client.calls],
                         ["insert", "upsert", "upsert", "patch"])
        self.assertEqual(client.calls[0][1].document.id, "1")
        self.assertEqual(client.calls[1][1].document.id, "1")
        self.assertEqual(client.calls[3][1].operations[0].kind, "set")


if __name__ == "__main__":
    unittest.main()
