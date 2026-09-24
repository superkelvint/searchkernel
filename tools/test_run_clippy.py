#!/usr/bin/env python3
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
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

    def test_cli_writes_complete_lint_repair_packet(self) -> None:
        diagnostic = {
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "code": {"code": "clippy::needless_borrow"},
                "message": "synthetic lint",
                "rendered": "error: synthetic lint\n",
                "spans": [
                    {
                        "suggested_replacement": "value",
                        "suggestion_applicability": "MachineApplicable",
                    }
                ],
            },
        }
        result, output_dir = self._run_with_fake_cargo(
            [
                "import json",
                "import sys",
                f"print(json.dumps({diagnostic!r}))",
                "raise SystemExit(1)",
            ]
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("CLIPPY_LINT_FAILURE", result.stdout)
        self.assertIn("./dev lint --profile rust-clippy-non-native", result.stdout)
        self.assertIn("error: synthetic lint", (output_dir / "clippy.log").read_text())
        summary = (output_dir / "summary.md").read_text()
        self.assertIn("`CLIPPY_LINT_FAILURE`", summary)
        self.assertIn("Machine-applicable suggestions: 1", summary)
        encoded = [
            json.loads(line)
            for line in (output_dir / "clippy-diagnostics.jsonl").read_text().splitlines()
        ]
        self.assertEqual(
            encoded[0]["message"]["code"]["code"], "clippy::needless_borrow"
        )

    def test_cli_classifies_startup_or_build_failure_without_clippy(self) -> None:
        result, output_dir = self._run_with_fake_cargo(
            [
                "import sys",
                "print('synthetic build failure')",
                "raise SystemExit(2)",
            ]
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("BUILD_OR_COMPILE_FAILURE", result.stdout)
        self.assertIn("./dev doctor", (output_dir / "summary.md").read_text())

    def test_success_is_pass(self) -> None:
        self.assertEqual(run_clippy.classify(0, 0), "PASS")

    def _run_with_fake_cargo(
        self, body: list[str]
    ) -> tuple[subprocess.CompletedProcess[str], Path]:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        cargo = root / "cargo"
        cargo.write_text("#!/usr/bin/env python3\n" + "\n".join(body) + "\n")
        cargo.chmod(0o755)
        output_dir = root / "packet"
        env = os.environ.copy()
        env["PATH"] = str(root) + os.pathsep + env.get("PATH", "")
        result = subprocess.run(
            [
                sys.executable,
                str(MODULE_PATH),
                "--profile",
                "rust-clippy-non-native",
                "--diagnostics-dir",
                str(output_dir),
            ],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        return result, output_dir


if __name__ == "__main__":
    unittest.main()
