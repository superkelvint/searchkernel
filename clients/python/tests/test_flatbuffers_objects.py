"""The Python protocol path is the flatc object API, not a second model."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Boolean, ClientError, Float, Group, Hit, Integer, Keyword, RankProfile, Tensor, TensorField, Text, agg, g, q  # noqa: E402
from searchkernel.client import _document, _python_value, value  # noqa: E402
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
from SearchKernel.V2.ErrorCode import ErrorCode  # noqa: E402
from SearchKernel.V2.GroupingBinaryOperator import GroupingBinaryOperator  # noqa: E402
from SearchKernel.V2.GroupingOperationKind import GroupingOperationKind  # noqa: E402
from SearchKernel.V2.MatchingMode import MatchingMode  # noqa: E402
from SearchKernel.V2.PrimitiveDataTypeKind import PrimitiveDataTypeKind  # noqa: E402
from SearchKernel.V2.QueryExprKind import QueryExprKind  # noqa: E402
from searchkernel.ergonomic import SearchBuilder  # noqa: E402


class FlatBuffersObjectApiTest(unittest.TestCase):
    def test_nested_grouping_uses_structural_all_scope(self):
        request = Group.by("category").output(agg.count()).subgroup(
            Group.by("brand").output(agg.count())
        ).to_request()

        root_operations = request.source.root.operations
        self.assertEqual(
            [operation.operationType for operation in root_operations],
            [
                GroupingOperationKind.GroupingGroupOperation,
                GroupingOperationKind.GroupingEachOperation,
            ],
        )
        parent_each = root_operations[1].operation
        self.assertEqual(
            [operation.operationType for operation in parent_each.operations],
            [
                GroupingOperationKind.GroupingOutputOperation,
                GroupingOperationKind.GroupingAllOperation,
            ],
        )
        nested_all = parent_each.operations[1].operation
        self.assertEqual(
            [operation.operationType for operation in nested_all.operations],
            [
                GroupingOperationKind.GroupingGroupOperation,
                GroupingOperationKind.GroupingEachOperation,
            ],
        )

    def test_ergonomics_follow_generated_enum_constants(self):
        members = {
            (PrimitiveDataTypeKind, "String"): 101,
            (PrimitiveDataTypeKind, "Int64"): 105,
            (PrimitiveDataTypeKind, "Float64"): 107,
            (PrimitiveDataTypeKind, "Bool"): 102,
            (MatchingMode, "Unspecified"): 200,
            (MatchingMode, "Text"): 201,
            (MatchingMode, "Exact"): 203,
            (GroupingBinaryOperator, "Add"): 301,
            (GroupingBinaryOperator, "Subtract"): 302,
            (GroupingBinaryOperator, "Multiply"): 303,
            (GroupingBinaryOperator, "Divide"): 304,
            (ErrorCode, "InvalidArgument"): 401,
        }
        original = {(enum, name): getattr(enum, name) for enum, name in members}
        try:
            for (enum, name), value_ in members.items():
                setattr(enum, name, value_)

            self.assertEqual(Text().definition("title").dataType.kind.kind, PrimitiveDataTypeKind.String)
            self.assertEqual(Text().definition("title").matching.mode, MatchingMode.Text)
            self.assertEqual(Keyword().definition("sku").matching.mode, MatchingMode.Exact)
            self.assertEqual(Integer().definition("count").dataType.kind.kind, PrimitiveDataTypeKind.Int64)
            self.assertEqual(Float().definition("score").dataType.kind.kind, PrimitiveDataTypeKind.Float64)
            self.assertEqual(Boolean().definition("active").dataType.kind.kind, PrimitiveDataTypeKind.Bool)
            self.assertEqual((g.field("price") + 1).model.expr.op, GroupingBinaryOperator.Add)
            self.assertEqual((g.field("price") - 1).model.expr.op, GroupingBinaryOperator.Subtract)
            self.assertEqual((g.field("price") * 1).model.expr.op, GroupingBinaryOperator.Multiply)
            self.assertEqual((g.field("price") / 1).model.expr.op, GroupingBinaryOperator.Divide)
            self.assertEqual(
                ClientError("bad", error=SimpleNamespace(code=ErrorCode.InvalidArgument)).code,
                "invalid_argument",
            )
        finally:
            for (enum, name), value_ in original.items():
                setattr(enum, name, value_)

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

    def test_semantic_maps_use_canonical_key_order(self):
        self.assertEqual(
            [entry.key for entry in value({"zeta": 1, "alpha": 2}).kind.fields],
            ["alpha", "zeta"],
        )
        self.assertEqual(
            [entry.key for entry in _document("doc:1", {"zeta": 1, "alpha": 2}).fields],
            ["alpha", "zeta"],
        )

        class EmptyResult:
            root = None
            totalHits = None

        class CaptureClient:
            def search(self, request):
                self.request = request
                return EmptyResult()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, "wireless").property("zeta", 1).property("alpha", 2).execute()
        self.assertEqual(
            [entry.key for entry in index.client.request.properties],
            ["alpha", "zeta"],
        )

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

    def test_dense_tensor_values_round_trip_through_python_value(self):
        tensor = Tensor.dense("tensor<float>(x[2])", [1, 0])
        encoded = value(tensor)
        decoded = _python_value(encoded)
        self.assertEqual(
            decoded,
            {
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(x[2])",
                    "dense": [1.0, 0.0],
                }
            },
        )
        definition = TensorField("tensor<float>(x[2])", stored=True).definition("embedding")
        self.assertEqual(definition.dataType.kind.typeSpec, "tensor<float>(x[2])")

    def test_rank_profile_and_nearest_neighbor_lower_to_generated_objects(self):
        profile = (
            RankProfile("semantic")
            .first_phase("closeness(embedding)")
            .query_input("query_embedding", "tensor<float>(x[2])")
            .match_feature("query(query_embedding)")
            .definition()
        )
        self.assertEqual(profile.inputs[0].name, "query_embedding")
        self.assertEqual(profile.firstPhase.expression, "closeness(embedding)")

        class EmptyResult:
            root = None
            totalHits = None

        class CaptureClient:
            def search(self, request):
                self.request = request
                return EmptyResult()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(
            index, q.nearest_neighbor("embedding", "query_embedding")
        ).input(
            "query_embedding", Tensor.dense("tensor<float>(x[2])", [1, 0])
        ).execute()
        query = index.client.request.query.source.query
        self.assertEqual(query.exprType, QueryExprKind.QueryNearestNeighbor)
        self.assertEqual(query.expr.documentField.path, "embedding")
        self.assertEqual(query.expr.queryTensor, "query_embedding")

    def test_hit_preserves_scalar_and_opaque_features(self):
        from types import SimpleNamespace

        node = SimpleNamespace(
            id="doc:1",
            fields=[],
            relevance=None,
            matchFeatures=[
                __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="scalar", value=value(3.5)
                ),
                __import__("SearchKernel.V2.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="opaque", value=value(b"native-data")
                ),
            ],
            summaryFeatures=[],
            rankFeatures=[],
            sortValues=[value(7)],
        )
        hit = Hit(node)
        self.assertEqual(hit.features["scalar"], 3.5)
        self.assertEqual(hit.features["opaque"], b"native-data")
        self.assertEqual(hit.sort_values, [7])

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

    def test_ergonomic_rank_properties_use_default_profile(self):
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
            .rank_property("source", "catalog") \
            .execute()

        self.assertEqual(index.client.request.ranking.profile, "default")

    def test_ergonomic_ranking_properties_and_inputs_are_canonical_order(self):
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
            .rank_property("zeta", "last") \
            .rank_property("alpha", "first") \
            .input("zeta_input", 0.8) \
            .input("alpha_input", 0.2) \
            .execute()

        ranking = index.client.request.ranking
        self.assertEqual(ranking.profile, "product_search")
        self.assertEqual([entry.key for entry in ranking.properties], ["alpha", "zeta"])
        self.assertEqual(
            [entry.name for entry in index.client.request.inputs],
            ["alpha_input", "zeta_input"],
        )


if __name__ == "__main__":
    unittest.main()
