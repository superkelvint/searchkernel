"""The Python protocol path is the flatc object API, not a second model."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Group, agg, g  # noqa: E402
from searchkernel.client import value  # noqa: E402
from searchkernel.protocol import pack_object, pack_request  # noqa: E402
from SearchKernel.Protocol.V2.RequestEnvelope import RequestEnvelopeT  # noqa: E402
from SearchKernel.V2.Document import DocumentT  # noqa: E402
from SearchKernel.V2.InsertRequest import InsertRequestT  # noqa: E402
from SearchKernel.V2.TensorValue import TensorValueT  # noqa: E402
from SearchKernel.V2.TensorPayloadKind import TensorPayloadKind  # noqa: E402
from SearchKernel.V2.DenseTensorValues import DenseTensorValuesT  # noqa: E402
from SearchKernel.V2.IndexDefinition import IndexDefinitionT  # noqa: E402
from SearchKernel.V2.FilterClause import FilterClauseT  # noqa: E402
from SearchKernel.V2.FilterRequest import FilterRequestT  # noqa: E402
from SearchKernel.V2.SearchRequest import SearchRequestT  # noqa: E402
from searchkernel.ergonomic import SearchBuilder  # noqa: E402


class FlatBuffersObjectApiTest(unittest.TestCase):
    def test_recursive_value_and_envelope_round_trip(self):
        document = DocumentT(
            id="doc:1",
            fields=[],
        )
        document.fields = [
            __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                key="payload", value=value({"nested": [1, True]})
            )
        ]
        raw = pack_request("insert", InsertRequestT(document=document))
        envelope = RequestEnvelopeT.InitFromPackedBuf(raw)
        self.assertEqual(envelope.payload.document.id, "doc:1")
        nested = envelope.payload.document.fields[0].value.kind.fields[0].value.kind.values
        self.assertEqual([item.kind.value for item in nested], [1, True])

    def test_widened_v2_objects_pack_without_schema_mapper(self):
        tensor = TensorValueT(
            typeSpec="tensor<float>(x[2])",
            payloadType=TensorPayloadKind.DenseTensorValues,
            payload=DenseTensorValuesT(values=[0.25, 0.75]),
        )
        self.assertIsInstance(tensor, TensorValueT)

        grouping = Group.by("category").output(agg.count().as_("count"))
        request = grouping.to_request()
        raw = pack_object(request)
        self.assertGreater(len(raw), 0)

        expression = (g.field("price") + 1).model
        self.assertEqual(expression.expr.left.expr.field, "price")
        self.assertEqual(expression.expr.right.expr.value.kind.value, 1)

    def test_textual_filter_clauses_preserve_local_bindings(self):
        request = SearchRequestT(
            filters=FilterRequestT(
                textual=[
                    FilterClauseT(
                        source="price <= :value",
                        bindings=[
                            __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                                key="value", value=value(120)
                            )
                        ],
                    ),
                    FilterClauseT(
                        source="rating >= :value",
                        bindings=[
                            __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                                key="value", value=value(4)
                            )
                        ],
                    ),
                ]
            )
        )
        decoded = SearchRequestT.InitFromPackedBuf(pack_object(request))
        self.assertEqual(
            [clause.source for clause in decoded.filters.textual],
            ["price <= :value", "rating >= :value"],
        )
        self.assertEqual(
            [clause.bindings[0].key for clause in decoded.filters.textual],
            ["value", "value"],
        )

    def test_ergonomic_filter_bindings_are_sorted(self):
        class EmptyResult:
            root = None
            totalHits = None

        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return EmptyResult()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, None).filter("price <= :z", {"z": 1, "a": 2}).execute()
        self.assertEqual(
            [entry.key for entry in index.client.request.filters.textual[0].bindings],
            ["a", "z"],
        )

    def test_ergonomic_ranking_inputs_use_generated_objects(self):
        class EmptyResult:
            root = None
            totalHits = None

        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return EmptyResult()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, "wireless") \
            .ranking("product_search") \
            .input("freshness", 0.8) \
            .rank_property("source", "catalog") \
            .execute()
        request = index.client.request
        self.assertEqual(request.ranking.profile, "product_search")
        self.assertEqual(request.ranking.properties[0].value, "catalog")
        self.assertEqual(request.inputs[0].name, "freshness")
        self.assertEqual(request.inputs[0].input.value.kind.value, 0.8)


if __name__ == "__main__":
    unittest.main()
