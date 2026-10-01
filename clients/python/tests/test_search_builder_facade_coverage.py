"""Executable coverage for the normative SearchBuilder facade surface."""

from __future__ import annotations

import math
import re
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel.ergonomic import SearchBuilder, YqlBuilder  # noqa: E402
from searchkernel.protocol import pack_object  # noqa: E402
from searchkernel._semantic import GroupingSourceKind  # noqa: E402
from searchkernel._semantic import MissingPolicyKind  # noqa: E402
from searchkernel._semantic import QueryInputKind  # noqa: E402
from searchkernel._semantic import SortTargetKind  # noqa: E402


# This is intentionally exhaustive for docs/client-api-spec.md SearchBuilder.
# Non-executable rows remain represented at the facade/protocol boundary so the
# shared resolver/adapter owns their structured rejection rather than Python.
FACADE_COVERAGE = {
    "filter": ("executable", None),
    "search_field": ("executable_with_limit", "non-default weights are backend-deferred"),
    "search_fields": ("executable", None),
    "sort": ("executable", None),
    "sort_relevance": ("executable", None),
    "sort_feature": ("executable", None),
    "raw_sort": ("represented_backend_rejected", "raw sort has no canonical resolved form"),
    "facet": ("executable", None),
    "facets": ("executable", None),
    "aggregate": ("executable", None),
    "group": ("executable", None),
    "raw_grouping": ("represented_backend_rejected", "raw grouping normalization is deferred"),
    "ranking": ("executable", None),
    "rank_property": ("executable", None),
    "input": ("executable_with_limit", "native adapter executes finite scalar numeric and dense tensor inputs; other values are rejected"),
    "input_expr": ("represented_backend_rejected", "expression inputs require a container-aware path"),
    "fields": ("executable", None),
    "summary": ("executable", None),
    "highlight": ("executable", None),
    "format": ("represented_backend_rejected", "advanced presentation format is backend-deferred"),
    "bolding": ("represented_backend_rejected", "advanced presentation bolding is backend-deferred"),
    "timing": ("represented_backend_rejected", "advanced presentation timing is backend-deferred"),
    "offset": ("executable", None),
    "limit": ("executable", None),
    "page": ("executable", None),
    "timeout": ("executable", None),
    "language": ("executable", None),
    "locale": ("represented_backend_rejected", "native adapter supports language but not locale"),
    "default_index": ("executable", None),
    "sources": ("represented_backend_rejected", "source selection requires a container-aware path"),
    "restrict": ("represented_backend_rejected", "document restriction requires a container-aware path"),
    "search_path": ("represented_backend_rejected", "search path requires a container-aware path"),
    "search_group": ("represented_backend_rejected", "search group requires a container-aware path"),
    "query_string": ("executable", None),
    "query_model_type": ("represented_backend_rejected", "model type controls are backend-deferred"),
    "property": ("executable_with_limit", "generic properties execute for YQL substitution; non-YQL properties are rejected"),
    "properties": ("executable_with_limit", "generic properties execute for YQL substitution; non-YQL properties are rejected"),
    "query_profile": ("represented_backend_rejected", "query profiles require a container-aware path"),
    "search_chain": ("represented_backend_rejected", "search chains require a container-aware path"),
    "execute": ("executable", None),
    "inspect": ("executable", None),
}


class SearchBuilderFacadeCoverageTest(unittest.TestCase):
    def test_every_normative_concept_is_exposed_and_classified(self):
        spec = (Path(__file__).resolve().parents[3] / "docs" / "client-api-spec.md").read_text()
        conceptual = spec.split("## 15. Conceptual surface", 1)[1].split("# Part V — Filtering", 1)[0]
        normative = set(re.findall(r"^\.([a-z_]+)\(.*\)$", conceptual, flags=re.MULTILINE))
        self.assertEqual(set(FACADE_COVERAGE), normative)

        allowed = {"executable", "executable_with_limit", "represented_backend_rejected"}
        for concept, (status, reason) in FACADE_COVERAGE.items():
            with self.subTest(concept=concept):
                self.assertTrue(hasattr(SearchBuilder, concept), concept)
                self.assertIn(status, allowed)
                if status != "executable":
                    self.assertTrue(reason)

        partial = {
            concept
            for concept, (status, _reason) in FACADE_COVERAGE.items()
            if status == "executable_with_limit"
        }
        self.assertEqual(partial, {"search_field", "input", "property", "properties"})

    def test_explicit_empty_search_fields_preserves_presence(self):
        request = SearchBuilder(SimpleNamespace(), "wireless").search_fields()._request()
        self.assertIsNotNone(request.searchFields)
        self.assertEqual(request.searchFields.fields, [])

    def test_added_controls_lower_mechanically_to_generated_request(self):
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
        request = (
            SearchBuilder(index, "wireless")
            .search_field("title", weight=1.0)
            .search_fields("description")
            .sort("price", "asc", missing="last")
            .sort_relevance("desc")
            .sort_feature("freshness", "asc")
            .raw_sort("[rank]", "desc")
            .facets("brand", "category")
            .facet("seller", precision=200)
            .raw_grouping("all(group(category) each(output(count())))")
            .input_expr("query_embedding", "embed(transformer,@query)")
            .rank_properties({"alpha": "1", "zeta": "2"})
            .format("json")
            .bolding(True)
            .timing(False)
            .timeout(500)
            .language("en")
            .locale("en-US")
            .default_index("title")
            .sources("products")
            .restrict("products")
            .search_path("/search")
            .search_group(2)
            .query_string("wireless")
            .query_model_type(composite="weakAnd", syntax="advanced", yql_default=False)
            .properties({"zeta": 2, "alpha": 1})
            .query_profile("profile")
            .search_chain("chain")
            ._request()
        )

        self.assertEqual([field.field for field in request.searchFields.fields], ["title", "description"])
        self.assertTrue(math.isclose(request.searchFields.fields[0].weight.value, 1.0))
        self.assertIsNone(request.searchFields.fields[1].weight)

        self.assertEqual(
            [sort.target.targetType for sort in request.sort],
            [
                SortTargetKind.FieldSortTarget,
                SortTargetKind.RelevanceSortTarget,
                SortTargetKind.RankFeatureSortTarget,
                SortTargetKind.RawSortTarget,
            ],
        )
        self.assertEqual(request.sort[0].missing.policyType, MissingPolicyKind.MissingLast)
        self.assertEqual(request.sort[2].target.target.feature, "freshness")
        self.assertEqual(request.sort[3].target.target.expression, "[rank]")

        self.assertEqual(request.inputs[0].inputType, QueryInputKind.ExpressionQueryInput)
        self.assertEqual(request.inputs[0].input.expression, "embed(transformer,@query)")
        self.assertEqual(request.timeoutMs.value, 500)
        self.assertEqual(request.queryModel.language, "en")
        self.assertEqual(request.queryModel.locale, "en-US")
        self.assertEqual(request.queryModel.defaultIndex, "title")
        self.assertEqual(request.queryModel.sources, ["products"])
        self.assertEqual(request.queryModel.restrict, ["products"])
        self.assertEqual(request.queryModel.searchPath, "/search")
        self.assertEqual(request.queryModel.searchGroup.value, 2)
        self.assertEqual(request.queryModel.queryString, "wireless")
        self.assertEqual(request.queryModel.type.composite, "weakAnd")
        self.assertEqual(request.queryModel.type.syntax, "advanced")
        self.assertFalse(request.queryModel.type.isYqlDefault.value)
        self.assertEqual(request.presentation.format, "json")
        self.assertTrue(request.presentation.bolding.value)
        self.assertFalse(request.presentation.timing.value)
        self.assertEqual(request.queryProfile, "profile")
        self.assertEqual(request.searchChain, "chain")
        self.assertEqual([entry.key for entry in request.ranking.properties], ["alpha", "zeta"])
        self.assertEqual([entry.key for entry in request.properties], ["alpha", "zeta"])
        self.assertGreater(len(pack_object(request)), 0)

        raw_grouping = request.grouping[-1]
        self.assertEqual(raw_grouping.sourceType, GroupingSourceKind.RawGroupingSource)
        self.assertIn("group(category)", raw_grouping.source.expression)

        facet = request.grouping[2].source.root.operations[1].operation.operations
        self.assertTrue(any(op.operation.value == 200 for op in facet if hasattr(op.operation, "value")))

    def test_advanced_sort_and_yql_property_controls_preserve_caller_intent(self):
        missing_as = SearchBuilder(SimpleNamespace(), "wireless").sort(
            "price", "asc", missing=0
        )._request()
        self.assertEqual(
            missing_as.sort[0].missing.policyType,
            MissingPolicyKind.MissingAs,
        )

        yql = (
            YqlBuilder(SimpleNamespace(), "select * from sources * where title contains text(@q)")
            .property("q", "running shoes")
            .properties({"min_price": 10})
            ._request()
        )
        self.assertEqual([entry.key for entry in yql.properties], ["min_price", "q"])

    def test_rank_properties_convenience_lowers_to_normative_rank_property_state(self):
        request = (
            SearchBuilder(SimpleNamespace(), "wireless")
            .rank_properties({"zeta": "2", "alpha": "1"})
            ._request()
        )
        self.assertEqual([entry.key for entry in request.ranking.properties], ["alpha", "zeta"])

    def test_builder_inspect_uses_same_request_assembly_without_search_execution(self):
        class Index:
            def __init__(self):
                self.client = SimpleNamespace(search=lambda _request: self.fail("search should not run"))
                self.request = None

            def inspect_query(self, request):
                self.request = request
                return SimpleNamespace(resolvedRequest="resolved")

            def fail(self, message):
                raise AssertionError(message)

        index = Index()
        resolved = SearchBuilder(index, "wireless").timeout(50).language("en").inspect()
        self.assertEqual(resolved, "resolved")
        self.assertEqual(index.request.timeoutMs.value, 50)
        self.assertEqual(index.request.queryModel.language, "en")


if __name__ == "__main__":
    unittest.main()
