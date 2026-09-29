"""Regression coverage for lossless public search-result metadata."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel.client import value  # noqa: E402
from searchkernel.ergonomic import SearchResults  # noqa: E402
from SearchKernel.V2.FiniteRelevance import FiniteRelevanceT  # noqa: E402
from SearchKernel.V2.OptionalBool import OptionalBoolT  # noqa: E402
from SearchKernel.V2.OptionalDouble import OptionalDoubleT  # noqa: E402
from SearchKernel.V2.Relevance import RelevanceT  # noqa: E402
from SearchKernel.V2.RelevanceKind import RelevanceKind  # noqa: E402
from SearchKernel.V2.ResultGroup import ResultGroupT  # noqa: E402
from SearchKernel.V2.ResultLimits import ResultLimitsT  # noqa: E402
from SearchKernel.V2.ResultNode import ResultNodeT  # noqa: E402
from SearchKernel.V2.ResultNodeKind import ResultNodeKind  # noqa: E402
from SearchKernel.V2.SearchResult import SearchResultT  # noqa: E402
from SearchKernel.V2.SearchResultError import SearchResultErrorT  # noqa: E402
from SearchKernel.V2.SearchTiming import SearchTimingT  # noqa: E402
from SearchKernel.V2.StringEntry import StringEntryT  # noqa: E402
from SearchKernel.V2.TotalHits import TotalHitsT  # noqa: E402
from SearchKernel.V2.TotalHitsRelation import TotalHitsRelation  # noqa: E402
from SearchKernel.V2.ValueEntry import ValueEntryT  # noqa: E402


class ResultMetadataTest(unittest.TestCase):
    def test_public_results_preserve_envelope_and_group_metadata(self):
        group = ResultGroupT(
            id="group-id",
            label="group-label",
            value=value("bucket"),
            fields=[ValueEntryT("count", value(3))],
            continuations=[StringEntryT("cursor", "token-1")],
            source="source-a",
            types=["type-a"],
            relevance=RelevanceT(
                RelevanceKind.FiniteRelevance,
                FiniteRelevanceT(2.5),
            ),
            limits=ResultLimitsT(from_=value(1), to=value(10)),
            metadata=[ValueEntryT("group-meta", value("yes"))],
        )
        root = ResultGroupT(
            children=[ResultNodeT(ResultNodeKind.ResultGroup, group)]
        )
        result = SearchResults(
            SearchResultT(
                root=ResultNodeT(ResultNodeKind.ResultGroup, root),
                totalHits=TotalHitsT(value=0, relation=TotalHitsRelation.Exact),
                timing=SearchTimingT(
                    queryTimeSeconds=OptionalDoubleT(0.25),
                    metadata=[ValueEntryT("timing-meta", value("yes"))],
                ),
                errors=[
                    SearchResultErrorT(
                        code=7,
                        summary="summary",
                        message="message",
                        source="source-b",
                        stackTrace="trace-b",
                        transient=OptionalBoolT(True),
                        metadata=[ValueEntryT("error-meta", value("yes"))],
                    )
                ],
                trace=value("trace-value"),
                metadata=[ValueEntryT("result-meta", value("yes"))],
            )
        )

        self.assertEqual(result.timing.query_time_seconds, 0.25)
        self.assertEqual(result.timing.metadata, {"timing-meta": "yes"})
        self.assertEqual(result.errors[0].code, 7)
        self.assertEqual(result.errors[0].summary, "summary")
        self.assertEqual(result.errors[0].message, "message")
        self.assertEqual(result.errors[0].source, "source-b")
        self.assertEqual(result.errors[0].stack_trace, "trace-b")
        self.assertTrue(result.errors[0].transient)
        self.assertEqual(result.errors[0].metadata, {"error-meta": "yes"})
        self.assertEqual(result.trace, "trace-value")
        self.assertEqual(result.metadata, {"result-meta": "yes"})

        self.assertEqual(result.groups[0].id, "group-id")
        self.assertEqual(result.groups[0].source, "source-a")
        self.assertEqual(result.groups[0].types, ["type-a"])
        self.assertEqual(result.groups[0].relevance, 2.5)
        self.assertEqual(result.groups[0].limits.from_, 1)
        self.assertEqual(result.groups[0].limits.to, 10)
        self.assertEqual(result.groups[0].metadata, {"group-meta": "yes"})
        self.assertEqual(
            [(entry.key, entry.value) for entry in result.groups[0].continuations],
            [("cursor", "token-1")],
        )


if __name__ == "__main__":
    unittest.main()
