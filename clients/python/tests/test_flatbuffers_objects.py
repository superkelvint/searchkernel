"""The Python protocol path is the flatc object API, not a second model."""

from __future__ import annotations

import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Boolean, ClientError, Float, Group, Hit, HnswIndexConfig, Integer, Keyword, RankProfile, StructField, Tensor, TensorField, Text, agg, g, gf, q  # noqa: E402
from searchkernel.client import _document, _python_value, value  # noqa: E402
from searchkernel.ergonomic import (  # noqa: E402
    SearchBuilder,
    _facet_request,
    _feature_entries,
    _node,
    _root_aggregation_request,
    _value_entries,
)
from searchkernel.protocol import expected_response_type, pack_object, pack_request  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.RequestEnvelope import RequestEnvelopeT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.RequestPayload import RequestPayload  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Protocol.ResponsePayload import ResponsePayload  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.DeleteIndexRequest import DeleteIndexRequestT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.Document import DocumentT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.InsertRequest import InsertRequestT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.Int64Value import Int64ValueT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TensorValue import TensorValueT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TensorPayloadKind import TensorPayloadKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.DenseTensorValues import DenseTensorValuesT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.MixedTensorBlocks import MixedTensorBlocksT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.NonFiniteFloatKind import NonFiniteFloatKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.NonFiniteFloatValue import NonFiniteFloatValueT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SparseTensorCells import SparseTensorCellsT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TensorAddressEntry import TensorAddressEntryT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TensorBlock import TensorBlockT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TensorCell import TensorCellT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.UInt64Value import UInt64ValueT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.Value import ValueT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry import ValueEntryT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ValueKind import ValueKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.DistanceMetric import DistanceMetric  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.HnswIndexConfig import HnswIndexConfigT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.IndexDefinition import IndexDefinitionT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.FilterClause import FilterClauseT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.FilterRequest import FilterRequestT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SearchRequest import SearchRequestT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SortDirection import SortDirection  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SortExpr import SortExprT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SortTarget import SortTargetT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.SortTargetKind import SortTargetKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.FieldSortTarget import FieldSortTargetT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.MatchAllQuery import MatchAllQueryT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.QuerySource import QuerySourceT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.QuerySourceKind import QuerySourceKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ErrorCode import ErrorCode  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.GroupingBinaryOperator import GroupingBinaryOperator  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.GroupingFilterExprKind import GroupingFilterExprKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.GroupingOperationKind import GroupingOperationKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.GroupingExpressionKind import GroupingExpressionKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.MatchingMode import MatchingMode  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.PrimitiveDataTypeKind import PrimitiveDataTypeKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.QueryExprKind import QueryExprKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.Relevance import RelevanceT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.RelevanceKind import RelevanceKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.FiniteRelevance import FiniteRelevanceT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.PositiveInfinityRelevance import PositiveInfinityRelevanceT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.NegativeInfinityRelevance import NegativeInfinityRelevanceT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultNodeKind import ResultNodeKind  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultGroup import ResultGroupT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultNode import ResultNodeT  # noqa: E402
from searchkernel._wire.v1.generated.SearchKernel.Wire.TotalHitsRelation import TotalHitsRelation  # noqa: E402


_SNAPSHOT_BINARY = None


def _empty_search_result():
    return SimpleNamespace(
        root=ResultNodeT(ResultNodeKind.ResultGroup, ResultGroupT()),
        totalHits=SimpleNamespace(value=0, relation=TotalHitsRelation.Exact),
    )


def _snapshot_binary(root):
    global _SNAPSHOT_BINARY
    if _SNAPSHOT_BINARY is None:
        subprocess.run(
            [
                "cargo",
                "build",
                "--quiet",
                "--no-default-features",
                "-p",
                "searchkernel-testkit",
                "--bin",
                "searchkernel-semantic-snapshot",
            ],
            cwd=root,
            check=True,
        )
        metadata = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
        target = Path(json.loads(metadata.stdout)["target_directory"])
        _SNAPSHOT_BINARY = target / "debug" / "searchkernel-semantic-snapshot"
    return _SNAPSHOT_BINARY


def _canonical_snapshot(request):
    root = Path(__file__).parents[3]
    fixture = root / "reference/api-conformance/fixtures/indexes/product-v2.json"
    with tempfile.TemporaryDirectory() as directory:
        request_path = Path(directory) / "request.bfbs"
        request_path.write_bytes(pack_object(request))
        result = subprocess.run(
            [
                str(_snapshot_binary(root)),
                "--index-json",
                str(fixture),
                "--request",
                str(request_path),
            ],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
    return json.loads(result.stdout)


class FlatBuffersObjectApiTest(unittest.TestCase):
    def test_delete_index_uses_canonical_envelope_discriminators_and_target(self):
        raw = pack_request("delete_index", DeleteIndexRequestT(), target="catalog-a")
        envelope = RequestEnvelopeT.InitFromPackedBuf(raw)

        self.assertEqual(
            envelope.payloadType,
            RequestPayload.RequestPayload.SearchKernel_Wire_DeleteIndexRequest,
        )
        self.assertEqual(envelope.target.name, "catalog-a")
        self.assertEqual(
            expected_response_type("delete_index"),
            ResponsePayload.ResponsePayload.SearchKernel_Wire_DeleteIndexResult,
        )

    def test_signed_value_checks_int64_range_before_flatbuffers_packing(self):
        for item in (-(2**63) - 1, 2**63):
            with self.subTest(item=item):
                with self.assertRaisesRegex(ValueError, "int64"):
                    pack_object(value(item))

        for item in (-(2**63), 2**63 - 1, 0, 2**32 - 1):
            with self.subTest(item=item):
                encoded = value(item)
                self.assertEqual(encoded.kind.value, item)
                self.assertGreater(len(pack_object(encoded)), 0)

    def test_python_uint32_facades_reject_values_outside_wire_range(self):
        for item in (-1, 2**32):
            with self.subTest(kind="grouping max", item=item):
                with self.assertRaisesRegex(ValueError, "uint32"):
                    pack_object(Group.by("category").limit(item).to_request())
            with self.subTest(kind="grouping precision", item=item):
                with self.assertRaisesRegex(ValueError, "uint32"):
                    pack_object(Group.by("category").precision(item).to_request())
            with self.subTest(kind="HNSW max links", item=item):
                with self.assertRaisesRegex(ValueError, "uint32"):
                    HnswIndexConfig(max_links_per_node=item)._generated()
            with self.subTest(kind="HNSW neighbors", item=item):
                with self.assertRaisesRegex(ValueError, "uint32"):
                    HnswIndexConfig(neighbors_to_explore_at_insert=item)._generated()
            with self.subTest(kind="facet limit", item=item):
                with self.assertRaisesRegex(ValueError, "uint32"):
                    SearchBuilder(object(), None).facet("category", limit=item)
                with self.assertRaisesRegex(ValueError, "uint32"):
                    _facet_request("category", "category", item, None)

        self.assertGreater(
            len(pack_object(Group.by("category").limit(2**32 - 1).precision(2**32 - 1).to_request())),
            0,
        )
        config = HnswIndexConfig(
            max_links_per_node=2**32 - 1,
            neighbors_to_explore_at_insert=2**32 - 1,
        )._generated()
        self.assertEqual(config.maxLinksPerNode.value, 2**32 - 1)
        self.assertEqual(config.neighborsToExploreAtInsert.value, 2**32 - 1)

    def test_field_sort_requires_an_explicit_direction(self):
        class Index:
            client = None

        with self.assertRaises(TypeError):
            SearchBuilder(Index(), "wireless").sort("price")

        self.assertEqual(
            SearchBuilder(Index(), "wireless").sort("price", "asc")._sort[0].direction,
            SortDirection.Asc,
        )
        self.assertEqual(
            SearchBuilder(Index(), "wireless").sort("price", "desc")._sort[0].direction,
            SortDirection.Desc,
        )

    def test_field_sorts_match_canonical_rust_ir(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        for direction, enum_value in (("asc", SortDirection.Asc), ("desc", SortDirection.Desc)):
            index = Index()
            SearchBuilder(index, None).sort("price", direction).execute()
            canonical = SearchRequestT(
                query=QuerySourceT(
                    sourceType=QuerySourceKind.MatchAllQuery,
                    source=MatchAllQueryT(),
                ),
                sort=[SortExprT(
                    target=SortTargetT(
                        targetType=SortTargetKind.FieldSortTarget,
                        target=FieldSortTargetT(field="price"),
                    ),
                    direction=enum_value,
                )],
            )
            with self.subTest(direction=direction):
                self.assertEqual(
                    _canonical_snapshot(index.client.request),
                    _canonical_snapshot(canonical),
                )

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

    def test_ergonomics_use_version_free_semantic_enum_values(self):
        from searchkernel._semantic import SemanticEnumValue, SemanticTable

        definition = Text().definition("title")
        self.assertIsInstance(definition, SemanticTable)
        self.assertIsInstance(definition.dataType.kind.kind, SemanticEnumValue)
        self.assertEqual(definition.dataType.kind.kind, PrimitiveDataTypeKind.String)
        self.assertEqual(definition.matching.mode, MatchingMode.Text)
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

    def test_recursive_value_and_envelope_round_trip(self):
        document = DocumentT(
            id="doc:1",
            fields=[],
        )
        document.fields = [
            __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                key="payload", value=value({"nested": [1, True]})
            )
        ]
        raw = pack_request("insert", InsertRequestT(document=document))
        envelope = RequestEnvelopeT.InitFromPackedBuf(raw)
        self.assertEqual(envelope.payload.document.id, "doc:1")
        nested = envelope.payload.document.fields[0].value.kind.fields[0].value.kind.values
        self.assertEqual([item.kind.value for item in nested], [1, True])

    def test_request_target_round_trip(self):
        raw = pack_request(
            "insert",
            InsertRequestT(document=DocumentT(id="doc:1", fields=[])),
            target="catalog-a",
        )
        envelope = RequestEnvelopeT.InitFromPackedBuf(raw)
        self.assertEqual(envelope.target.name, "catalog-a")

    def test_semantic_maps_use_canonical_key_order(self):
        self.assertEqual(
            [entry.key for entry in value({"zeta": 1, "alpha": 2}).kind.fields],
            ["alpha", "zeta"],
        )
        self.assertEqual(
            [entry.key for entry in _document("doc:1", {"zeta": 1, "alpha": 2}).fields],
            ["alpha", "zeta"],
        )

        class CaptureClient:
            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, "wireless").property("zeta", 1).property("alpha", 2).execute()
        self.assertEqual(
            [entry.key for entry in index.client.request.properties],
            ["alpha", "zeta"],
        )

    def test_facet_preserves_zero_limit_and_canonical_name(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, None).facet(
            "brand", name="brands", limit=0
        ).execute()
        root = index.client.request.grouping[0].source.root
        each = root.operations[1].operation

        self.assertEqual(each.operations[0].operation.value.value, 0)
        self.assertEqual(root.label, "brands")

    def test_facet_sort_modes_and_multiple_names_are_canonical(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        builder = SearchBuilder(index, None)
        sorts = ["count_desc", "count_asc", "value_asc", "value_desc"]
        expected_kinds = [
            GroupingExpressionKind.GroupingUnaryExpression,
            GroupingExpressionKind.GroupingAggregationExpression,
            GroupingExpressionKind.GroupingAttributeExpression,
            GroupingExpressionKind.GroupingUnaryExpression,
        ]
        for position, sort in enumerate(sorts):
            builder.facet(
                "brand" if position % 2 == 0 else "category",
                name=f"facet-{position}",
                limit=position,
                sort=sort,
            )
        builder.execute()

        self.assertEqual(len(index.client.request.grouping), 4)
        for position, grouping in enumerate(index.client.request.grouping):
            root = grouping.source.root
            each = root.operations[1].operation
            self.assertEqual(root.label, f"facet-{position}")
            self.assertEqual(each.operations[0].operation.value.value, position)
            self.assertEqual(
                each.operations[1].operation.expressions[0].expression.exprType,
                expected_kinds[position],
            )

    def test_ergonomic_summary_and_fields_share_one_presentation(self):
        class CaptureClient:
            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, "wireless").summary("default").fields("title").highlight("title").execute()
        presentation = index.client.request.presentation
        self.assertEqual(presentation.summary, "default")
        self.assertEqual(presentation.fields.fields, ["title"])
        self.assertEqual(presentation.highlighting.fields, ["title"])

    def test_wire_codec_packs_version_free_semantic_objects(self):
        from searchkernel._semantic import DenseTensorValuesT as SemanticDenseTensorValuesT
        from searchkernel._semantic import SemanticTable, TensorValueT as SemanticTensorValueT

        tensor = SemanticTensorValueT(
            typeSpec="tensor<float>(x[2])",
            payloadType=TensorPayloadKind.DenseTensorValues,
            payload=SemanticDenseTensorValuesT(values=[0.25, 0.75]),
        )
        self.assertIsInstance(tensor, SemanticTable)

        grouping = Group.by("category").output(agg.count().as_("count"))
        request = grouping.to_request()
        raw = pack_object(request)
        self.assertGreater(len(raw), 0)

        expression = (g.field("price") + 1).model
        self.assertEqual(expression.expr.left.expr.field, "price")
        self.assertEqual(expression.expr.right.expr.value.kind.value, 1)

    def test_rank_profile_preserves_second_phase_and_rerank_count(self):
        definition = (
            RankProfile("two-phase")
            .first_phase("attribute(rating)")
            .second_phase("attribute(inventory)")
            .rerank_count(3)
            .definition()
        )

        self.assertEqual(definition.firstPhase.expression, "attribute(rating)")
        self.assertEqual(definition.secondPhase.expression, "attribute(inventory)")
        self.assertEqual(definition.secondPhase.rerankCount.value, 3)
        self.assertGreater(len(pack_object(definition)), 0)

        with self.assertRaisesRegex(ValueError, "uint32"):
            RankProfile("two-phase").rerank_count(0x1_0000_0000)


    def test_struct_field_preserves_capability_options(self):
        definition = StructField(
            {"first": Keyword(), "last": Keyword()},
            stored=True,
            searchable=False,
            filterable=True,
            sortable=False,
            facetable=True,
        ).definition("person")
        self.assertTrue(definition.stored.value)
        self.assertFalse(definition.searchable.value)
        self.assertTrue(definition.filterable.value)
        self.assertFalse(definition.sortable.value)
        self.assertTrue(definition.facetable.value)
        self.assertEqual([field.name for field in definition.dataType.kind.fields], ["first", "last"])

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

    def test_result_value_decoder_preserves_nonfinite_and_tensor_variants(self):
        self.assertIsNone(_python_value(value(None)))
        self.assertEqual(
            _python_value(
                ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=2**63))
            ),
            2**63,
        )
        for kind, expected in (
            (NonFiniteFloatKind.NaN, math.nan),
            (NonFiniteFloatKind.PositiveInfinity, math.inf),
            (NonFiniteFloatKind.NegativeInfinity, -math.inf),
        ):
            decoded = _python_value(
                ValueT(
                    kindType=ValueKind.NonFiniteFloatValue,
                    kind=NonFiniteFloatValueT(kind=kind),
                )
            )
            # Keep the public Value helper out of this regression: non-finite
            # scalars are response-only values and are intentionally rejected
            # when constructing request values.
            if math.isnan(expected):
                self.assertTrue(math.isnan(decoded))
            else:
                self.assertEqual(decoded, expected)

        sparse = _python_value(
            ValueT(
                kindType=ValueKind.TensorValue,
                kind=TensorValueT(
                    typeSpec="tensor<float>(x{})",
                    payloadType=TensorPayloadKind.SparseTensorCells,
                    payload=SparseTensorCellsT(
                        cells=[
                            TensorCellT(
                                address=[TensorAddressEntryT(dimension="x", label="a")],
                                value=0.5,
                            )
                        ]
                    ),
                ),
            )
        )
        mixed = _python_value(
            ValueT(
                kindType=ValueKind.TensorValue,
                kind=TensorValueT(
                    typeSpec="tensor<float>(x{},y[2])",
                    payloadType=TensorPayloadKind.MixedTensorBlocks,
                    payload=MixedTensorBlocksT(
                        blocks=[
                            TensorBlockT(
                                address=[TensorAddressEntryT(dimension="x", label="a")],
                                values=[0.25, 0.75],
                            )
                        ]
                    ),
                ),
            )
        )
        self.assertEqual(sparse["$searchkernel_tensor"]["sparse"][0]["value"], 0.5)
        self.assertEqual(mixed["$searchkernel_tensor"]["mixed"][0]["values"], [0.25, 0.75])

    def test_result_value_decoder_rejects_malformed_tensor_state(self):
        malformed_addresses = [
            TensorAddressEntryT(dimension=None, label="a"),
            TensorAddressEntryT(dimension="x", label=None),
        ]
        for address in malformed_addresses:
            with self.subTest(address=address):
                with self.assertRaisesRegex(ValueError, "malformed tensor address"):
                    _python_value(
                        ValueT(
                            kindType=ValueKind.TensorValue,
                            kind=TensorValueT(
                                typeSpec="tensor<float>(x{})",
                                payloadType=TensorPayloadKind.SparseTensorCells,
                                payload=SparseTensorCellsT(
                                    cells=[TensorCellT(address=[address], value=0.5)]
                                ),
                            ),
                        )
                    )

        for payload_type, payload in (
            (
                TensorPayloadKind.DenseTensorValues,
                DenseTensorValuesT(values=[math.nan]),
            ),
            (
                TensorPayloadKind.SparseTensorCells,
                SparseTensorCellsT(
                    cells=[
                        TensorCellT(
                            address=[TensorAddressEntryT(dimension="x", label="a")],
                            value=math.inf,
                        )
                    ]
                ),
            ),
            (
                TensorPayloadKind.MixedTensorBlocks,
                MixedTensorBlocksT(
                    blocks=[
                        TensorBlockT(
                            address=[TensorAddressEntryT(dimension="x", label="a")],
                            values=[-math.inf],
                        )
                    ]
                ),
            ),
        ):
            with self.subTest(payload_type=payload_type):
                with self.assertRaisesRegex(ValueError, "tensor cells must be finite"):
                    _python_value(
                        ValueT(
                            kindType=ValueKind.TensorValue,
                            kind=TensorValueT(
                                typeSpec="tensor<float>(x{})",
                                payloadType=payload_type,
                                payload=payload,
                            ),
                        )
                    )

    def test_result_decoding_rejects_ambiguous_and_unknown_unions(self):
        entries = [
            ValueEntryT(key="duplicate", value=value(1)),
            ValueEntryT(key="duplicate", value=value(2)),
        ]
        with self.assertRaisesRegex(ValueError, "duplicate key"):
            _value_entries(entries)

        with self.assertRaisesRegex(ValueError, "unknown Value union"):
            _python_value(ValueT(kindType=255))
        with self.assertRaisesRegex(ValueError, "mismatched Value union"):
            _python_value(
                ValueT(
                    kindType=ValueKind.BytesValue,
                    kind=Int64ValueT(value=1),
                )
            )
        with self.assertRaisesRegex(ValueError, "unknown ResultNode union"):
            _node(SimpleNamespace(nodeType=255, node=SimpleNamespace()))
        with self.assertRaisesRegex(ValueError, "missing ResultNode payload"):
            _node(SimpleNamespace(nodeType=ResultNodeKind.Hit, node=None))

    def test_feature_value_lists_are_not_reinterpreted_as_bytes(self):
        entry = ValueEntryT(key="numbers", value=value([1, 2]))
        self.assertEqual(_feature_entries([entry]), {"numbers": [1, 2]})

    def test_tensor_field_hnsw_configuration_lowers_to_canonical_objects(self):
        definition = TensorField(
            "tensor<float>(x[3])",
            distance_metric="euclidean",
            hnsw=HnswIndexConfig(
                max_links_per_node=4,
                neighbors_to_explore_at_insert=16,
            ),
        ).definition("embedding")
        self.assertEqual(definition.attribute.distanceMetric, DistanceMetric.Euclidean)
        self.assertEqual(definition.index.hnsw.maxLinksPerNode.value, 4)
        self.assertEqual(definition.index.hnsw.neighborsToExploreAtInsert.value, 16)

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

        class CaptureClient:
            def search(self, request):
                self.request = request
                return _empty_search_result()

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

    def test_nearest_neighbor_controls_lower_to_existing_annotations(self):
        query = q.nearest_neighbor(
            "embedding",
            "query_embedding",
            approximate=False,
            total_target_hits=3,
            explore_additional_hits=5,
            distance_threshold=100.0,
        )
        annotations = {entry.key: entry.value.kind.value for entry in query.source.query.annotations}
        self.assertEqual(annotations["approximate"], False)
        self.assertEqual(annotations["totalTargetHits"], 3)
        self.assertEqual(annotations["exploreAdditionalHits"], 5)
        self.assertEqual(annotations["distanceThreshold"], 100.0)

    def test_hybrid_query_combines_text_and_nearest_neighbor(self):
        query = q.all_of(
            q.contains("title", "shoes"),
            q.nearest_neighbor("embedding", "query_embedding"),
        )

        self.assertEqual(query.source.query.exprType, QueryExprKind.QueryAnd)
        self.assertEqual(
            [clause.exprType for clause in query.source.query.expr.clauses],
            [QueryExprKind.QueryContains, QueryExprKind.QueryNearestNeighbor],
        )

    def test_typed_query_helpers_lower_to_generated_objects(self):
        queries = [
            q.contains("title", q.phrase("quick", "fox")),
            q.contains("title", q.near("quick", "fox")),
            q.contains("title", q.onear("quick", "fox")),
            q.contains("title", q.fuzzy("foxx")),
            q.same_element(q.contains("first", "Ada"), q.contains("last", "Lovelace")),
            q.wand("title", {"fox": 1, "quick": 10}),
            q.rank(q.contains("title", "fox"), q.contains("title", "quick")),
            q.geo_location("location", lat=37.3, lon=-121.9, radius="10km"),
            q.geo_box("location", south=37.0, west=-122.0, north=38.0, east=-121.0),
        ]
        self.assertEqual(
            [query.source.query.exprType for query in queries],
            [
                QueryExprKind.QueryContains,
                QueryExprKind.QueryContains,
                QueryExprKind.QueryContains,
                QueryExprKind.QueryContains,
                QueryExprKind.QuerySameElement,
                QueryExprKind.QueryWand,
                QueryExprKind.QueryRank,
                QueryExprKind.QueryGeoLocation,
                QueryExprKind.QueryGeoBoundingBox,
            ],
        )
        self.assertEqual(
            queries[5].source.query.expr.items[0].key.kind.value,
            "fox",
        )

    # SKC: query.prefix_suffix_substring/basic kinds=ergonomic runtime=shared
    def test_literal_term_prefix_substring_suffix_helpers_lower_canonically(self):
        term = q.term("title", "quick")
        self.assertEqual(term.source.query.exprType, QueryExprKind.QueryContains)
        self.assertEqual(term.source.query.expr.item.exprType, QueryExprKind.TermQuery)

        cases = [
            (q.prefix("brand", "A.c+"), r"A\.c\+.*"),
            (q.substring("brand", "A.c+"), r".*A\.c\+.*"),
            (q.suffix("brand", "A.c+"), r".*A\.c\+"),
        ]
        for query, pattern in cases:
            self.assertEqual(query.source.query.exprType, QueryExprKind.QueryMatches)
            self.assertEqual(query.source.query.expr.pattern, pattern)

    def test_grouping_filter_boolean_helpers_have_readable_names(self):
        clause = gf.is_true(g.field("in_stock"))

        self.assertEqual(
            gf.all_of(clause, clause).exprType,
            GroupingFilterExprKind.GroupingFilterAnd,
        )
        self.assertEqual(
            gf.any_of(clause, clause).exprType,
            GroupingFilterExprKind.GroupingFilterOr,
        )
        self.assertEqual(
            gf.negate(clause).exprType,
            GroupingFilterExprKind.GroupingFilterNot,
        )

    def test_hit_preserves_scalar_and_opaque_features(self):
        from types import SimpleNamespace

        node = SimpleNamespace(
            id="doc:1",
            fields=[],
            relevance=None,
            matchFeatures=[
                __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="scalar", value=value(3.5)
                ),
                __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
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

    def test_hit_preserves_source_types_feature_maps_and_relevance_kinds(self):
        base = dict(
            id="doc:1",
            fields=[],
            source="catalog",
            types=["product", "featured"],
            matchFeatures=[
                __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="match", value=value(["one", True])
                )
            ],
            summaryFeatures=[
                __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="summary", value=value({"ready": True})
                )
            ],
            rankFeatures=[
                __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                    key="rank", value=value(7)
                )
            ],
            sortValues=[value("a"), value(9)],
            highlights=[],
        )

        finite = Hit(SimpleNamespace(
            **base,
            relevance=RelevanceT(
                valueType=RelevanceKind.FiniteRelevance,
                value=FiniteRelevanceT(value=2.5),
            ),
        ))
        positive = Hit(SimpleNamespace(
            **base,
            relevance=RelevanceT(
                valueType=RelevanceKind.PositiveInfinityRelevance,
                value=PositiveInfinityRelevanceT(),
            ),
        ))
        negative = Hit(SimpleNamespace(
            **base,
            relevance=RelevanceT(
                valueType=RelevanceKind.NegativeInfinityRelevance,
                value=NegativeInfinityRelevanceT(),
            ),
        ))

        self.assertEqual(finite.source, "catalog")
        self.assertEqual(finite.types, ["product", "featured"])
        self.assertEqual(finite.match_features["match"], ["one", True])
        self.assertEqual(finite.summary_features["summary"], {"ready": True})
        self.assertEqual(finite.rank_features["rank"], 7)
        self.assertEqual(finite.sort_values, ["a", 9])
        self.assertEqual(finite.relevance, 2.5)
        self.assertEqual(positive.relevance, float("inf"))
        self.assertEqual(negative.relevance, float("-inf"))

    def test_hit_preserves_structured_highlights(self):
        from types import SimpleNamespace

        node = SimpleNamespace(
            id="doc:1",
            fields=[],
            relevance=None,
            matchFeatures=[],
            summaryFeatures=[],
            rankFeatures=[],
            sortValues=[],
            highlights=[
                SimpleNamespace(
                    field="title",
                    fragments=[
                        SimpleNamespace(
                            text="quick fox",
                            matches=[SimpleNamespace(startByte=6, endByte=9)],
                        )
                    ],
                )
            ],
        )
        self.assertEqual(
            Hit(node).highlights,
            [{"field": "title", "fragments": [{"text": "quick fox", "matches": [(6, 9)]}] }],
        )

    def test_textual_filter_clauses_preserve_local_bindings(self):
        request = SearchRequestT(
            filters=FilterRequestT(
                textual=[
                    FilterClauseT(
                        source="price <= :value",
                        bindings=[
                            __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
                                key="value", value=value(120)
                            )
                        ],
                    ),
                    FilterClauseT(
                        source="rating >= :value",
                        bindings=[
                            __import__("searchkernel._wire.v1.generated.SearchKernel.Wire.ValueEntry", fromlist=["ValueEntryT"]).ValueEntryT(
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
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

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
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

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

    def test_ergonomic_rank_property_rejects_non_string_values(self):
        class Index:
            client = None

        for item in (False, None, 7, {}, []):
            with self.subTest(item=item):
                with self.assertRaises(TypeError):
                    SearchBuilder(Index(), "wireless").rank_property("enabled", item)

        builder = SearchBuilder(Index(), "wireless").rank_property("source", "catalog")
        self.assertEqual(builder._rank_properties, {"source": "catalog"})

    def test_ergonomic_rank_properties_preserve_absent_profile(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        index = Index()
        SearchBuilder(index, "wireless") \
            .rank_property("source", "catalog") \
            .execute()

        ranking = index.client.request.ranking
        self.assertIsNone(ranking.profile)
        self.assertEqual(ranking.properties[0].value, "catalog")

        SearchBuilder(index, "wireless").ranking("default") \
            .rank_property("source", "catalog") \
            .execute()
        self.assertEqual(index.client.request.ranking.profile, "default")

    def test_ergonomic_ranking_properties_and_inputs_are_canonical_order(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

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

    def test_page_and_grouping_helpers_match_canonical_rust_ir(self):
        class CaptureClient:
            def __init__(self):
                self.request = None

            def search(self, request):
                self.request = request
                return _empty_search_result()

        class Index:
            def __init__(self):
                self.client = CaptureClient()

        page_index = Index()
        SearchBuilder(page_index, None).page(2, 20).execute()
        explicit_index = Index()
        SearchBuilder(explicit_index, None).limit(20).offset(20).execute()
        self.assertEqual(
            _canonical_snapshot(page_index.client.request),
            _canonical_snapshot(explicit_index.client.request),
        )
        first_page_index = Index()
        SearchBuilder(first_page_index, None).page(1, 20).execute()
        first_page_explicit_index = Index()
        SearchBuilder(first_page_explicit_index, None).limit(20).offset(0).execute()
        self.assertEqual(
            _canonical_snapshot(first_page_index.client.request),
            _canonical_snapshot(first_page_explicit_index.client.request),
        )
        zero_page_index = Index()
        SearchBuilder(zero_page_index, None).page(1, 0).execute()
        zero_explicit_index = Index()
        SearchBuilder(zero_explicit_index, None).limit(0).offset(0).execute()
        self.assertEqual(
            _canonical_snapshot(zero_page_index.client.request),
            _canonical_snapshot(zero_explicit_index.client.request),
        )

        helper_index = Index()
        SearchBuilder(helper_index, None).facet(
            "brand", name="brands", limit=5, sort="count_desc"
        ).aggregate(agg.count().as_("products")).group(
            Group.by("category").limit(3).output(agg.count().as_("products"))
        ).execute()
        explicit = SearchRequestT(
            query=QuerySourceT(
                sourceType=QuerySourceKind.MatchAllQuery,
                source=MatchAllQueryT(),
            ),
            grouping=[
                Group.by("category").limit(3).output(agg.count().as_("products")).to_request(),
                _facet_request("brand", "brands", 5, "count_desc"),
                _root_aggregation_request([agg.count().as_("products")]),
            ],
        )
        self.assertEqual(
            _canonical_snapshot(helper_index.client.request),
            _canonical_snapshot(explicit),
        )

    def test_page_rejects_non_uint32_and_conflicting_pagination(self):
        class Index:
            class Client:
                def search(self, _request):
                    raise AssertionError("invalid pagination must fail before transport")

            client = Client()

        for number, size in [
            (0, 10),
            (1, -1),
            (1, 0x1_0000_0000),
            (2, 0xFFFF_FFFF),
            (0x1_0000_0000, 1),
        ]:
            with self.subTest(number=number, size=size):
                with self.assertRaises(ValueError):
                    SearchBuilder(Index(), None).page(number, size)
        with self.assertRaises(ValueError):
            SearchBuilder(Index(), None).page(2, 10).limit(10)
        with self.assertRaises(ValueError):
            SearchBuilder(Index(), None).limit(0x1_0000_0000)


if __name__ == "__main__":
    unittest.main()
