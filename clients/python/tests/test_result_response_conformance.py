import json
import math
import os
import unittest
from pathlib import Path

from searchkernel.ergonomic import Hit, ResultGroup, SearchResults
from searchkernel.protocol import unpack_response
from searchkernel._semantic import is_semantic_table

# The version-neutral protocol helper delegates to its private wire codec.
# root on sys.path. Keep generated-wire imports after that production bootstrap.
from searchkernel._wire.v1.generated.SearchKernel.Wire.SearchResult import SearchResultT
from searchkernel._wire.v1.generated.SearchKernel.Wire.TotalHitsRelation import TotalHitsRelation


def _value(value):
    if value is None or isinstance(value, (bool, str)):
        return value
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        if math.isnan(value):
            return "nan"
        if math.isinf(value):
            return "+inf" if value > 0 else "-inf"
        return value
    if isinstance(value, (bytes, bytearray)):
        return [str(item) for item in value]
    if isinstance(value, (list, tuple)):
        return [_value(item) for item in value]
    if isinstance(value, dict):
        return {key: _value(item) for key, item in sorted(value.items())}
    raise TypeError(f"unsupported trace value: {type(value).__name__}")


def _relevance(value):
    return _value(value)


def _node(node):
    if isinstance(node, Hit):
        return {
            "kind": "hit",
            "id": node.id,
            "relevance": _relevance(node.relevance),
            "fields": _value(node.fields),
            "highlights": [
                {
                    "field": highlight["field"],
                    "fragments": [
                        {
                            "text": fragment["text"],
                            "matches": [
                                {
                                    "start_byte": str(match[0]),
                                    "end_byte": str(match[1]),
                                }
                                for match in fragment["matches"]
                            ],
                        }
                        for fragment in highlight["fragments"]
                    ],
                }
                for highlight in node.highlights
            ],
            "source": node.source,
            "types": list(node.types),
            "match_features": _value(node.match_features),
            "summary_features": _value(node.summary_features),
            "rank_features": _value(node.rank_features),
            "sort_values": _value(node.sort_values),
        }
    if isinstance(node, ResultGroup):
        return {
            "kind": "group",
            "id": node.id,
            "label": node.label,
            "value": _value(node.value),
            "fields": _value(node.fields),
            "children": [_node(child) for child in node.children],
            "continuations": [
                {"key": item.key, "value": item.value} for item in node.continuations
            ],
            "source": node.source,
            "types": list(node.types),
            "relevance": _relevance(node.relevance),
            "limits": (
                None
                if node.limits is None
                else {"from": _value(node.limits.from_), "to": _value(node.limits.to)}
            ),
            "metadata": _value(node.metadata),
        }
    if node is None:
        return None
    raise TypeError(f"unsupported result node: {type(node).__name__}")


def _relation(value):
    names = {
        TotalHitsRelation.Unspecified: "unspecified",
        TotalHitsRelation.Exact: "exact",
        TotalHitsRelation.LowerBound: "lower_bound",
        TotalHitsRelation.Approximate: "approximate",
        TotalHitsRelation.Unknown: "unknown",
    }
    return names.get(value, "unspecified")


def _coverage(value):
    if value is None:
        return None
    degraded = value.degraded
    return {
        "coverage_percent": _value(value.coverage_percent),
        "documents": _value(value.documents),
        "full": value.full,
        "nodes": _value(value.nodes),
        "results": _value(value.results),
        "results_full": _value(value.results_full),
        "degraded": (
            None
            if degraded is None
            else {
                "match_phase": degraded.match_phase,
                "timeout": degraded.timeout,
                "adaptive_timeout": degraded.adaptive_timeout,
                "ann_timeout": degraded.ann_timeout,
                "non_ideal_state": degraded.non_ideal_state,
                "metadata": _value(degraded.metadata),
            }
        ),
        "metadata": _value(value.metadata),
    }


def _timing(value):
    if value is None:
        return None
    return {
        "query_time_seconds": _value(value.query_time_seconds),
        "search_time_seconds": _value(value.search_time_seconds),
        "summary_fetch_time_seconds": _value(value.summary_fetch_time_seconds),
        "metadata": _value(value.metadata),
    }


def _issue(value):
    return {
        "code": str(value.code),
        "summary": value.summary,
        "message": value.message,
        "source": value.source,
        "stack_trace": value.stack_trace,
        "transient": value.transient,
        "metadata": _value(value.metadata),
    }


def _trace(result):
    return {
        "root": _node(result.root),
        "total_hits": {
            "value": str(result.total_hits),
            "relation": _relation(result.total_hits_relation),
        },
        "coverage": _coverage(result.coverage),
        "timing": _timing(result.timing),
        "errors": [_issue(issue) for issue in result.errors],
        "trace": _value(result.trace),
        "metadata": _value(result.metadata),
    }


@unittest.skipUnless(os.getenv("SEARCHKERNEL_RESULT_CASES"), "run through ./dev verifier")
class ResultResponseConformance(unittest.TestCase):
    def test_production_response_bytes(self):
        cases = json.loads(Path(os.environ["SEARCHKERNEL_RESULT_CASES"]).read_text())
        case_directory = Path(os.environ["SEARCHKERNEL_RESULT_CASE_DIR"])
        traces = {}
        for case in cases["valid"]:
            with self.subTest(scenario=case["id"]):
                encoded = (case_directory / case["file"]).read_bytes()
                envelope = unpack_response(encoded, "search")
                self.assertTrue(is_semantic_table(envelope.payload, "SearchResult"))
                result = SearchResults(envelope.payload)
                self.assertIsNotNone(result.root)
                traces[case["id"]] = _trace(result)

        outcomes = {}
        for case in cases["malformed"]:
            with self.subTest(malformed=case["id"]):
                encoded = (
                    Path(os.environ["SEARCHKERNEL_RESULT_FIXTURE_DIR"]) / case["file"]
                ).read_bytes()
                with self.assertRaises((ValueError, TypeError, IndexError)):
                    envelope = unpack_response(encoded, "search")
                    SearchResults(envelope.payload)
                outcomes[case["id"]] = "client"

        traces["malformed_outcomes"] = outcomes
        Path(os.environ["SEARCHKERNEL_RESULT_TRACE_DIR"], "python.json").write_text(
            json.dumps(traces, sort_keys=True)
        )


if __name__ == "__main__":
    unittest.main()
