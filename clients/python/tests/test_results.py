import unittest
from types import SimpleNamespace

from searchkernel.client import value
from searchkernel.ergonomic import Hit, SearchBuilder, SearchResults
from SearchKernel.V2.ResultNodeKind import ResultNodeKind
from SearchKernel.V2.UInt64Value import UInt64ValueT
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.ValueEntry import ValueEntryT
from SearchKernel.V2.ValueKind import ValueKind


def result_node(node):
    return SimpleNamespace(nodeType=ResultNodeKind.ResultGroup, node=node)


def group(*, label=None, value_=None, fields=None, children=None):
    return SimpleNamespace(
        id=None,
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
    return SimpleNamespace(root=result_node(root), totalHits=None, coverage=None, errors=[])


def uint64_count(count):
    return ValueEntryT(
        key="count",
        value=ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=count)),
    )


class SearchResultFacetProjectionTest(unittest.TestCase):
    def exact_facet_result(self, *, label="brand", count=None, mixed=False):
        count = count or uint64_count(3)
        bucket = group(value_=value("Acme"), fields=[count])
        children = [result_node(bucket)]
        if mixed:
            hit = SimpleNamespace(
                id="doc-1",
                fields=[],
                relevance=None,
                matchFeatures=[],
                summaryFeatures=[],
                rankFeatures=[],
                sortValues=[],
                highlights=[],
            )
            children.append(SimpleNamespace(nodeType=ResultNodeKind.Hit, node=hit))
        facet = group(label=label, children=children)
        wrapper = group(children=[result_node(facet)])
        root = group(children=[result_node(wrapper)])
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
        self.assertEqual(result.root.children[0].children[0].label, "brand")

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


if __name__ == "__main__":
    unittest.main()
