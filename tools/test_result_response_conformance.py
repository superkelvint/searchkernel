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
                "case-a": {"root": {"kind": "hit", "id": "a"}},
                "case-b": {"root": {"kind": "hit", "id": "b"}},
            }
            for language in languages
        }
        return languages, traces

    def test_perturbed_decoded_trace_fails_comparison(self):
        _, traces = self.fixtures()
        traces["php"]["case-a"] = {"root": {"kind": "hit", "id": "wrong"}}
        with self.assertRaisesRegex(
            SystemExit, r"normalized result trace mismatch: case-a \(php\)"
        ):
            MODULE.compare(
                traces,
                [{"id": "case-a"}, {"id": "case-b"}],
                validate_shapes=False,
            )

    def test_missing_client_scenario_fails_comparison(self):
        _, traces = self.fixtures()
        del traces["ruby"]["case-b"]
        with self.assertRaisesRegex(SystemExit, "ruby result trace coverage mismatch"):
            MODULE.compare(
                traces,
                [{"id": "case-a"}, {"id": "case-b"}],
                validate_shapes=False,
            )

    def test_empty_map_becoming_empty_list_fails_comparison(self):
        _, traces = self.fixtures()
        expected = {"kind": "group", "fields": {}, "metadata": {}}
        for language in traces:
            traces[language]["case-a"] = expected.copy()
        traces["php"]["case-a"] = {"kind": "group", "fields": [], "metadata": {}}
        with self.assertRaisesRegex(SystemExit, "normalized result trace mismatch: case-a \\(php\\)"):
            MODULE.compare(traces, [{"id": "case-a"}, {"id": "case-b"}], validate_shapes=False)

    def test_integer_precision_loss_fails_comparison(self):
        _, traces = self.fixtures()
        maximum = 2**64 - 1
        for language in traces:
            traces[language]["case-a"] = {"value": maximum}
        traces["typescript"]["case-a"] = {"value": float(maximum)}
        with self.assertRaisesRegex(SystemExit, "normalized result trace mismatch: case-a \\(typescript\\)"):
            MODULE.compare(traces, [{"id": "case-a"}, {"id": "case-b"}], validate_shapes=False)

    def test_malformed_bridge_stage_is_distinguished_from_client_rejection(self):
        languages = ("rust", "python", "typescript", "php", "ruby")
        outputs = {
            language: {
                "malformed_outcomes": {
                    "bridge-case": "bridge" if language in {"php", "ruby"} else "client",
                    "client-case": "client",
                }
            }
            for language in languages
        }
        MODULE.verify_malformed(
            outputs,
            [
                {"id": "bridge-case", "bridge_rejected": True},
                {"id": "client-case", "bridge_rejected": False},
            ],
        )

    def test_malformed_bridge_case_cannot_be_claimed_as_client_rejection(self):
        languages = ("rust", "python", "typescript", "php", "ruby")
        outputs = {
            language: {
                "malformed_outcomes": {
                    "bridge-case": "client",
                }
            }
            for language in languages
        }
        with self.assertRaisesRegex(SystemExit, "php malformed outcome mismatch"):
            MODULE.verify_malformed(
                outputs,
                [{"id": "bridge-case", "bridge_rejected": True}],
            )


if __name__ == "__main__":
    unittest.main()
