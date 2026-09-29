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
        self.assertIn(
            "Hit has unclassified schema fields: future_result_field",
            MODULE.verify(schema),
        )

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


if __name__ == "__main__":
    unittest.main()
