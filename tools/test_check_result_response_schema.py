from __future__ import annotations

import copy
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tools/check-result-response-schema.py"
SPEC = importlib.util.spec_from_file_location("result_response_schema", SOURCE)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ResultResponseSchemaTest(unittest.TestCase):
    def test_checked_in_response_bfbs_is_the_schema_authority(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        self.assertEqual(MODULE.verify(MODULE.BFBS.read_bytes()), [])
        self.assertIn("TensorCell", closure)
        self.assertIn("ValueKind", closure)

    def test_corrupt_non_bfbs_input_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            MODULE.result_closure(b"not a BFBS reflection schema")

    def test_synthetic_union_discriminators_are_not_logical_fields(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        self.assertEqual(set(closure["ResultNode"][1]), {"node"})
        self.assertEqual(set(closure["Value"][1]), {"kind"})
        self.assertNotIn("NONE", closure["ResultNodeKind"][1])
        self.assertEqual(set(closure["ResultNodeKind"][1]), {"Hit", "ResultGroup"})

    def test_new_hit_field_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["Hit"][1]["future_result_field"] = "future_result_field"
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("Hit members mismatch" in error for error in errors))

    def test_new_reachable_result_type_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["FutureResultMetadata"] = ("table", {})
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("result schema declarations mismatch" in error for error in errors))

    def test_new_deeply_nested_field_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["TensorCell"][1]["future_cell_field"] = "future_cell_field"
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("TensorCell members mismatch" in error for error in errors))

    def test_new_total_hits_relation_value_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["TotalHitsRelation"][1]["FutureRelation"] = 5
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("TotalHitsRelation members mismatch" in error for error in errors))

    def test_new_result_node_union_arm_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["ResultNodeKind"][1]["FutureNode"] = 3
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("ResultNodeKind members mismatch" in error for error in errors))

    @staticmethod
    def manifest():
        return MODULE.json.loads(MODULE.MANIFEST.read_text())

    def test_classified_field_must_be_populated_by_its_scenario(self) -> None:
        envelope = {
            "payload": {
                "root": {
                    "node_type": "ResultGroup",
                    "node": {
                        "id": "root",
                        "label": "Root",
                        "value": {},
                        "fields": [],
                        "children": [
                            {
                                "node_type": "Hit",
                                "node": {
                                    field: object()
                                    for field in MODULE.CLASSIFIED_FIELDS["Hit"]
                                    if field != "highlights"
                                },
                            }
                        ],
                        "continuations": [],
                        "source": "source",
                        "types": [],
                        "relevance": {},
                        "limits": {},
                        "metadata": [],
                    },
                },
                "total_hits": {},
                "coverage": {},
                "timing": {},
                "errors": [],
                "trace": {},
                "metadata": [],
            }
        }
        errors = MODULE.verify_fixture_coverage({"canonical": envelope})
        self.assertIn(
            "Hit.highlights is not populated by scenario canonical",
            errors,
        )

    def test_manifest_scenario_must_really_populate_field(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["fields"]["Hit"]["highlights"] = ["empty"]
        errors = MODULE.verify_partitions(manifest, {"empty": {"payload": {}}})
        self.assertTrue(any("Hit.highlights is not exercised" in error for error in errors))

    def test_malformed_manifest_scenario_must_match_failure_class(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["semantic"]["malformed.unknown_value_union"] = [
            "unknown-result-node-union"
        ]
        errors = MODULE.verify_partitions(manifest, {})
        self.assertTrue(
            any(
                "malformed.unknown_value_union scenario unknown-result-node-union"
                in error
                for error in errors
            )
        )

    def test_manifest_missing_scenario_fails(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["semantic"]["test.missing"] = ["does-not-exist"]
        errors = MODULE.verify_partitions(manifest, {})
        self.assertIn("test.missing references missing scenario does-not-exist", errors)

    def test_trace_shape_requires_every_classified_field(self) -> None:
        trace = {
            field: None for field in MODULE.CLASSIFIED_FIELDS["SearchResult"]
        }
        trace["root"] = {
            "kind": "hit",
            **{
                field: None
                for field in MODULE.CLASSIFIED_FIELDS["Hit"]
                if field != "sort_values"
            },
        }
        errors = MODULE.trace_shape_errors(trace)
        self.assertTrue(any("sort_values" in error for error in errors))

    def test_trace_shape_checks_transitive_result_subobjects(self) -> None:
        trace = {
            "root": {
                "kind": "hit",
                **{field: {} if field in {"fields", "match_features", "summary_features", "rank_features"} else [] if field in {"highlights", "types", "sort_values"} else None for field in MODULE.CLASSIFIED_FIELDS["Hit"]},
            },
            "total_hits": {"value": "1", "relation": "exact"},
            "coverage": {"metadata": {}},
            "timing": None,
            "errors": [],
            "trace": None,
            "metadata": {},
        }
        errors = MODULE.trace_shape_errors(trace)
        self.assertTrue(any("coverage fields mismatch" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
