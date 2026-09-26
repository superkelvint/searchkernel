#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / ".github" / "ci" / "check-self-hosted-rust-workflows.py"
RUNNER = ROOT / ".github" / "workflows" / "ci-image-runner.yml"


class VerificationCachePolicyTests(unittest.TestCase):
    def run_fixture(self, workflow_extra: str = "", moon: bool = False, delegated_script: str = "") -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            runner = RUNNER.read_text(encoding="utf-8")
            (workflows / "ci-image-runner.yml").write_text(runner + workflow_extra, encoding="utf-8")
            if delegated_script:
                tools = root / "tools"
                tools.mkdir()
                (tools / "acceptance.sh").write_text(delegated_script, encoding="utf-8")
            if moon:
                moon_dir = root / ".moon"
                moon_dir.mkdir()
                (moon_dir / "tasks.yml").write_text("tasks:\n  test:\n    command: cargo test\n", encoding="utf-8")
            return subprocess.run(
                ["python3", str(CHECKER), str(root)], text=True, capture_output=True, check=False
            )

    def test_current_policy_passes(self) -> None:
        result = subprocess.run(["python3", str(CHECKER), str(ROOT)], text=True, capture_output=True)
        self.assertEqual(0, result.returncode, result.stderr)

    def test_rejects_nextest_archive_as_test_evidence(self) -> None:
        result = self.run_fixture("\n# cargo nextest run --archive-file stale.tar.zst\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("nextest archive reuse", result.stderr)

    def test_rejects_delegated_nextest_archive_reuse(self) -> None:
        result = self.run_fixture(delegated_script="#!/bin/sh\ncargo nextest archive --archive-file stale.tar.zst\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("forbidden delegated nextest archive reuse", result.stderr)

    def test_rejects_moon_task_execution(self) -> None:
        result = self.run_fixture("\n# moon run test\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("moon task-result execution", result.stderr)

    def test_rejects_moon_configuration_until_provenance_closure_exists(self) -> None:
        result = self.run_fixture(moon=True)
        self.assertNotEqual(0, result.returncode)
        self.assertIn("moon task-result configuration is forbidden", result.stderr)


if __name__ == "__main__":
    unittest.main()
