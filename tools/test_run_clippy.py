#!/usr/bin/env python3
import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("run_clippy.py")
SPEC = importlib.util.spec_from_file_location("run_clippy", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
run_clippy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(run_clippy)


class RunClippyTests(unittest.TestCase):
    def test_counts_clippy_and_machine_applicable_suggestion(self) -> None:
        message = {
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "code": {"code": "clippy::needless_borrow"},
                "spans": [
                    {
                        "suggested_replacement": "value",
                        "suggestion_applicability": "MachineApplicable",
                    }
                ],
            },
        }
        self.assertEqual(run_clippy.analyze_compiler_message(message), (1, 1, 1))
        self.assertEqual(run_clippy.classify(1, 1), "CLIPPY_LINT_FAILURE")

    def test_non_clippy_compile_error_is_not_mislabeled(self) -> None:
        message = {
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "code": {"code": "E0308"},
                "spans": [],
            },
        }
        self.assertEqual(run_clippy.analyze_compiler_message(message), (1, 0, 0))
        self.assertEqual(run_clippy.classify(1, 0), "BUILD_OR_COMPILE_FAILURE")

    def test_success_is_pass(self) -> None:
        self.assertEqual(run_clippy.classify(0, 0), "PASS")


if __name__ == "__main__":
    unittest.main()
