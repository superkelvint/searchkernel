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


if __name__ == "__main__":
    unittest.main()
