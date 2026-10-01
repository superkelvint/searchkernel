import math
import unittest
from types import SimpleNamespace
from pathlib import Path

from searchkernel.client import value
from searchkernel.ergonomic import Hit, SearchBuilder, SearchResults
from searchkernel.protocol import unpack_response
from SearchKernel.V2.Hit import HitT
from SearchKernel.V2.Int64Value import Int64ValueT
from SearchKernel.V2.ResultNodeKind import ResultNodeKind
from SearchKernel.V2.RelevanceKind import RelevanceKind
from SearchKernel.V2.ResultGroup import ResultGroupT
from SearchKernel.V2.ResultNode import ResultNodeT
from SearchKernel.V2.UInt64Value import UInt64ValueT
from SearchKernel.V2.TotalHitsRelation import TotalHitsRelation
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.ValueEntry import ValueEntryT
from SearchKernel.V2.ValueKind import ValueKind

CONFORMANCE_FIXTURE_ROOT = (
    Path(__file__).resolve().parents[3]
    / "reference/result-response-conformance/fixtures"
)


def result_node(node):
    node_type = ResultNodeKind.Hit if isinstance(node, HitT) else ResultNodeKind.ResultGroup
    return ResultNodeT(node_type, node)


def group(*, id_=None, label=None, value_=None, fields=None, children=None):
    return ResultGroupT(
        id=id_,
        label=label,
        value=value_,
        fields=list(fields or []),
        children=list(children or []),
        continuations=[],
        source=None,
        types=[],
        relevance=None,
        limits=None,
        metadata=[],
    )


def search_result(root):
    return SimpleNamespace(
        root=result_node(root),
        totalHits=SimpleNamespace(value=0, relation=TotalHitsRelation.Exact),
        coverage=None,
        errors=[],
    )


def uint64_count(count):
    return ValueEntryT(
        key="count",
        value=ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=count)),
    )


def int64_count(count):
    return ValueEntryT(
        key="count",
        value=ValueT(kindType=ValueKind.Int64Value, kind=Int64ValueT(value=count)),
    )


class SearchResultStructuralValidationTest(unittest.TestCase):
    def test_missing_root_and_total_hits_fail_closed(self):
        total_hits = SimpleNamespace(value=0, relation=TotalHitsRelation.Exact)
        with self.assertRaisesRegex(ValueError, "SearchResult.root"):
            SearchResults(SimpleNamespace(root=None, totalHits=total_hits))
        with self.assertRaisesRegex(ValueError, "SearchResult.total_hits"):
            SearchResults(SimpleNamespace(root=result_node(group()), totalHits=None))


class SearchResultFacetProjectionTest(unittest.TestCase):
    def exact_facet_result(
        self, *, label="brand", count=None, mixed=False, native=False
    ):
        count = count or uint64_count(3)
        bucket = group(
            id_="grouping-0" if native else None,
            label=label if native else None,
            value_=value("Acme"),
            fields=[count],
        )
        children = [result_node(bucket)]
        if mixed:
            hit = HitT(
                id="doc-1",
                fields=[],
                relevance=None,
                matchFeatures=[],
                summaryFeatures=[],
                rankFeatures=[],
                sortValues=[],
                highlights=[],
            )
            children.append(ResultNodeT(ResultNodeKind.Hit, hit))
        facet = group(label=label, children=children)
        root = group(children=[result_node(facet)])
        return search_result(root)

    def test_unlabeled_group_wrapper_is_not_flattened(self):
        bucket = group(label="bucket")
        wrapper = group(children=[result_node(bucket)])
        root = group(children=[result_node(wrapper)])

        result = SearchResults(search_result(root))

        self.assertEqual(len(result.groups), 1)
        self.assertIs(result.groups[0], result.root.children[0])
        self.assertIsNone(result.groups[0].label)
        self.assertEqual(result.groups[0].groups[0].label, "bucket")

    def test_generic_labeled_group_is_not_inferred_as_facet(self):
        result = SearchResults(self.exact_facet_result())
        self.assertEqual(result.facets, {})
        self.assertEqual(result.groups[0].label, "brand")
        self.assertEqual(result.root.children[0].label, "brand")

    def test_requested_exact_facet_projects_uint64_count(self):
        raw = self.exact_facet_result()

        class Client:
            def search(self, _request):
                return raw

        result = SearchBuilder(SimpleNamespace(client=Client()), None).facet("brand").execute()
        self.assertEqual(result.facets, {"brand": [{"value": "Acme", "count": 3}]})

    def test_named_requested_facet_uses_requested_result_name(self):
        raw = self.exact_facet_result(label="brands")

        class Client:
            def search(self, _request):
                return raw

        result = SearchBuilder(SimpleNamespace(client=Client()), None).facet(
            "brand", name="brands"
        ).execute()
        self.assertEqual(result.facets, {"brands": [{"value": "Acme", "count": 3}]})

    def test_requested_facet_projects_native_int64_count(self):
        raw = self.exact_facet_result(
            label="brands", count=int64_count(3), native=True
        )

        class Client:
            def search(self, _request):
                return raw

        result = (
            SearchBuilder(SimpleNamespace(client=Client()), None)
            .facet("brand", name="brands")
            .execute()
        )
        self.assertEqual(result.facets, {"brands": [{"value": "Acme", "count": 3}]})

        raw = self.exact_facet_result(
            label="brands", count=int64_count(-1), native=True
        )
        result = (
            SearchBuilder(SimpleNamespace(client=Client()), None)
            .facet("brand", name="brands")
            .execute()
        )
        self.assertEqual(result.facets, {})

    def test_missing_or_wrong_count_does_not_project(self):
        missing = self.exact_facet_result(count=ValueEntryT(key="other", value=value(3)))
        wrong_type = self.exact_facet_result(count=ValueEntryT(key="count", value=value(3)))

        for raw in (missing, wrong_type):
            results = SearchResults(raw)
            results._project_facet("brand", "brand")
            self.assertEqual(results.facets, {})

    def test_mixed_hit_group_children_do_not_project(self):
        results = SearchResults(self.exact_facet_result(mixed=True))
        results._project_facet("brand", "brand")
        self.assertEqual(results.facets, {})

    def test_facet_with_extra_metadata_does_not_project(self):
        bucket = group(value_=value("Acme"), fields=[uint64_count(3)])
        facet = group(label="brand", children=[result_node(bucket)])
        facet.metadata = [ValueEntryT("unexpected", value(True))]
        raw = search_result(group(children=[result_node(facet)]))

        results = SearchResults(raw)
        results._project_facet("brand", "brand")
        self.assertEqual(results.facets, {})


class RelevanceDecodeTest(unittest.TestCase):
    def _hit(self, relevance):
        return SimpleNamespace(
            id="doc-1",
            fields=[],
            relevance=relevance,
            matchFeatures=[],
            summaryFeatures=[],
            rankFeatures=[],
            sortValues=[],
            highlights=[],
        )

    def test_relevance_discriminator_requires_matching_payload(self):
        for kind in (
            RelevanceKind.FiniteRelevance,
            RelevanceKind.PositiveInfinityRelevance,
            RelevanceKind.NegativeInfinityRelevance,
        ):
            with self.subTest(kind=kind):
                with self.assertRaisesRegex(ValueError, "missing or mismatched payload"):
                    Hit(
                        self._hit(
                            SimpleNamespace(
                                valueType=kind,
                                value=None,
                            )
                        )
                    )


class SharedResultFixtureTest(unittest.TestCase):
    fixture_root = (
        Path(__file__).resolve().parents[3]
        / "core/crates/searchkernel-dispatch/tests/fixtures/portable/responses"
    )

    def load(self, name):
        envelope = unpack_response(
            (self.fixture_root / f"result-{name}.skrs").read_bytes(), "search"
        )
        return SearchResults(envelope.payload)

    # SKC: result.highlighting/basic kinds=conformance runtime=shared
    def test_canonical_fixture_preserves_tree_and_envelope(self):
        result = self.load("canonical")
        root = result.root

        self.assertEqual(root.id, "root")
        self.assertEqual([type(child).__name__ for child in root.children], ["Hit", "ResultGroup"])
        self.assertEqual([hit.id for hit in result.hits], ["doc-1"])
        self.assertEqual([group.id for group in result.groups], ["nested"])
        self.assertEqual([hit.id for hit in result.groups[0].hits], ["doc-2"])
        self.assertEqual(result.aggregations, {"count": 2})
        self.assertEqual(result.groups[0].fields, {"count": 1})
        self.assertEqual([(item.key, item.value) for item in root.continuations], [("page", "token-1")])
        self.assertEqual([(item.key, item.value) for item in result.groups[0].continuations], [("next", "token")])
        self.assertEqual(root.limits.from_, 0)
        self.assertEqual(root.limits.to, 20)
        self.assertEqual(root.metadata, {"trace_id": "r-1"})

        hit = result.hits[0]
        self.assertEqual(hit.relevance, 4.5)
        self.assertEqual(hit.highlights[0]["fragments"][0]["matches"], [(0, 5)])
        self.assertEqual(hit.match_features, {"bm25": 2.0})
        self.assertEqual(hit.summary_features, {"summary": "ready"})
        self.assertEqual(hit.rank_features, {"rank": 8.0})
        self.assertEqual(hit.sort_values, [1])
        self.assertEqual(result.total_hits, 12)
        self.assertEqual(result.coverage.coverage_percent, 99.0)
        self.assertEqual(result.coverage.metadata, {"region": "west"})
        self.assertEqual(result.timing.search_time_seconds, 0.002)
        self.assertEqual(result.timing.metadata, {"phase": "done"})
        self.assertEqual(result.errors[0].code, 503)
        self.assertEqual(result.errors[0].metadata, {"node": "2"})
        self.assertEqual(result.trace, {"level": 3})
        self.assertEqual(result.metadata, {"request_id": "r1"})

    def test_shared_fixture_preserves_infinite_relevance_and_empty_state(self):
        result = self.load("infinite-relevance")
        self.assertTrue(math.isinf(result.hits[1].relevance))
        self.assertGreater(result.hits[1].relevance, 0)
        self.assertTrue(math.isinf(result.hits[2].relevance))
        self.assertLess(result.hits[2].relevance, 0)

        empty = self.load("empty")
        self.assertEqual(empty.hits, [])
        self.assertEqual(empty.groups, [])
        self.assertIsNone(empty.coverage)
        self.assertIsNone(empty.timing)
        self.assertEqual(empty.metadata, {})

        root_hit = self.load("root-hit")
        self.assertEqual([hit.id for hit in root_hit.hits], ["root-hit"])
        self.assertEqual(root_hit.groups, [])

        state_only = self.load("root-state-only")
        self.assertEqual(
            [(entry.key, entry.value) for entry in state_only.continuations],
            [("page", "token")],
        )
        self.assertEqual(state_only.root.limits.from_, 1)
        self.assertEqual(state_only.root.metadata, {"state": "present"})

    def test_shared_fixture_preserves_sparse_and_mixed_tensors(self):
        result = self.load("sparse-mixed-tensors")
        hit = result.hits[0]
        self.assertEqual(
            hit["sparse"],
            {
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(label{},region{})",
                    "sparse": [
                        {
                            "address": [
                                {"dimension": "label", "label": "a"},
                                {"dimension": "region", "label": "east"},
                            ],
                            "value": 0.5,
                        },
                        {
                            "address": [
                                {"dimension": "label", "label": "b"},
                                {"dimension": "region", "label": "west"},
                            ],
                            "value": -0.25,
                        },
                    ],
                }
            },
        )
        self.assertEqual(
            result.root.value,
            {
                "$searchkernel_tensor": {
                    "type_spec": "tensor<float>(label{},region{},x[2])",
                    "mixed": [
                        {
                            "address": [
                                {"dimension": "label", "label": "b"},
                                {"dimension": "region", "label": "west"},
                            ],
                            "values": [0.25, 0.75],
                        },
                        {
                            "address": [
                                {"dimension": "label", "label": "c"},
                                {"dimension": "region", "label": "north"},
                            ],
                            "values": [1.5, -2.0],
                        },
                    ],
                }
            },
        )

    def test_shared_facet_corpus_matches_strict_rust_shape(self):
        valid = self.load("facet-valid")
        valid._project_facet("brand", "brand")
        self.assertEqual(valid.facets, {"brand": [{"value": "Acme", "count": 3}]})

        named = self.load("facet-valid-named")
        named._project_facet("brands", "brands", "brand")
        self.assertEqual(named.facets, {"brands": [{"value": "Acme", "count": 3}]})

        for path in self.fixture_root.glob("result-facet-*.skrs"):
            if path.stem in {"result-facet-valid", "result-facet-valid-named"}:
                continue
            result = self.load(path.stem.removeprefix("result-"))
            result._project_facet("brand", "brand")
            self.assertEqual(result.facets, {}, path.name)

    def test_shared_malformed_tensor_fixtures_fail_closed(self):
        for filename in (
            "result-malformed-non-finite-dense-tensor-cell.skrs",
            "result-malformed-non-finite-sparse-tensor-cell.skrs",
            "result-malformed-non-finite-mixed-tensor-cell.skrs",
        ):
            with self.subTest(filename=filename):
                envelope = unpack_response(
                    (CONFORMANCE_FIXTURE_ROOT / filename).read_bytes(), "search"
                )
                with self.assertRaisesRegex(ValueError, "finite"):
                    SearchResults(envelope.payload)


if __name__ == "__main__":
    unittest.main()
