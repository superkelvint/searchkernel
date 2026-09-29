from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "result_response_conformance", ROOT / "tools/check-result-response-conformance.py"
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ResultResponseRunnerTest(unittest.TestCase):
    def fixtures(self):
        languages = ("rust", "python", "typescript", "php", "ruby")
        traces = {
            language: {
                "case-a": json.dumps({"payload_type": "SearchKernel_V2_SearchResult", "payload": {"value": 1}}),
                "case-b": json.dumps({"payload_type": "SearchKernel_V2_SearchResult", "payload": {"value": 2}}),
            }
            for language in languages
        }
        return languages, traces

    def test_perturbed_normalized_trace_fails_comparison(self):
        languages, traces = self.fixtures()
        traces["php"]["case-a"] = json.dumps(
            {"payload_type": "SearchKernel_V2_SearchResult", "payload": {"value": 9}}
        )
        with self.assertRaisesRegex(SystemExit, "normalized result trace mismatch: case-a \(php\)"):
            MODULE.compare(traces, [{"id": "case-a"}, {"id": "case-b"}])

    def test_missing_client_scenario_fails_comparison(self):
        languages, traces = self.fixtures()
        del traces["ruby"]["case-b"]
        with self.assertRaisesRegex(SystemExit, "ruby result trace coverage mismatch"):
            MODULE.compare(traces, [{"id": "case-a"}, {"id": "case-b"}])
