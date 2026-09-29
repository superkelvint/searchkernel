from __future__ import annotations

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
    def test_current_response_fields_are_classified(self) -> None:
        self.assertEqual(MODULE.verify(MODULE.SCHEMA.read_text()), [])

    def test_new_field_fails_until_classified(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "table Hit {", "table Hit {\n  future_result_field:string;", 1
        )
        self.assertTrue(any("Hit members mismatch" in error for error in MODULE.verify(schema)))

    def test_new_nested_result_field_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "table TensorCell {", "table TensorCell {\n  future_cell_field:string;", 1
        )
        self.assertTrue(any("TensorCell members mismatch" in error for error in MODULE.verify(schema)))

    def test_new_enum_value_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "Unknown = 4", "Unknown = 4,\n  FutureRelation = 5", 1
        )
        self.assertTrue(any("TotalHitsRelation members mismatch" in error for error in MODULE.verify(schema)))

    def test_field_type_change_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "table Hit {\n  id:string;\n  relevance:Relevance;\n  fields:[ValueEntry];",
            "table Hit {\n  id:string;\n  relevance:Relevance;\n  fields:[StringEntry];",
            1,
        )
        self.assertTrue(any("wire members mismatch" in error for error in MODULE.verify(schema)))

    def test_enum_storage_type_change_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "enum TotalHitsRelation:ubyte {",
            "enum TotalHitsRelation:ushort {",
            1,
        )
        self.assertTrue(any("wire header mismatch" in error for error in MODULE.verify(schema)))

    def test_enum_numeric_identity_change_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace("Unknown = 4", "Unknown = 9", 1)
        self.assertTrue(any("wire members mismatch" in error for error in MODULE.verify(schema)))

    def test_union_reordering_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "union ResultNodeKind {\n  Hit,\n  ResultGroup\n}",
            "union ResultNodeKind {\n  ResultGroup,\n  Hit\n}",
            1,
        )
        self.assertTrue(any("wire members mismatch" in error for error in MODULE.verify(schema)))

    def test_new_union_arm_fails(self) -> None:
        schema = MODULE.SCHEMA.read_text().replace(
            "  ResultGroup\n}", "  ResultGroup,\n  FutureNode\n}", 1
        )
        self.assertTrue(any("ResultNodeKind members mismatch" in error for error in MODULE.verify(schema)))

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
        manifest = MODULE.json.loads(MODULE.MANIFEST.read_text())
        manifest["partitions"]["fields"]["Hit"]["highlights"] = ["empty"]
        errors = MODULE.verify_partitions(manifest, {"empty": {"payload": {}}})
        self.assertTrue(any("Hit.highlights is not exercised" in error for error in errors))

    def test_malformed_manifest_scenario_must_match_failure_class(self) -> None:
        manifest = MODULE.json.loads(MODULE.MANIFEST.read_text())
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
        manifest = MODULE.json.loads(MODULE.MANIFEST.read_text())
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
