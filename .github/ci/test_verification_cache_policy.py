#!/usr/bin/env python3
from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / ".github" / "ci" / "check-self-hosted-rust-workflows.py"
RUNNER = ROOT / ".github" / "ci" / "ci-image-runner.yml"


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

    def run_candidate_copy(self, checker_extra: str = "") -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as raw:
            candidate = Path(raw)
            for relative in (".github", "tools", "verification"):
                source = ROOT / relative
                if source.exists():
                    shutil.copytree(source, candidate / relative)
            if checker_extra:
                candidate_checker = (
                    candidate / ".github" / "ci" / "check-self-hosted-rust-workflows.py"
                )
                candidate_checker.write_text(
                    candidate_checker.read_text(encoding="utf-8") + checker_extra,
                    encoding="utf-8",
                )
            return subprocess.run(
                ["python3", str(CHECKER), str(candidate)],
                text=True,
                capture_output=True,
                check=False,
            )

    def test_trusted_checker_accepts_identical_candidate_copy(self) -> None:
        result = self.run_candidate_copy()
        self.assertEqual(0, result.returncode, result.stderr)

    def test_trusted_checker_rejects_moon_in_candidate_checker(self) -> None:
        result = self.run_candidate_copy(
            '\nimport os\nos.system("moon run test")\n'
        )
        self.assertNotEqual(0, result.returncode)
        self.assertIn("check-self-hosted-rust-workflows.py", result.stderr)
        self.assertIn("forbidden delegated task-result execution via Moon", result.stderr)

    def test_trusted_checker_rejects_nextest_in_candidate_checker(self) -> None:
        result = self.run_candidate_copy(
            '\nimport os\nos.system("cargo nextest archive --archive-file stale.tar.zst")\n'
        )
        self.assertNotEqual(0, result.returncode)
        self.assertIn("check-self-hosted-rust-workflows.py", result.stderr)
        self.assertIn("forbidden delegated archive reuse via nextest", result.stderr)

    def test_rejects_nextest_archive_as_test_evidence(self) -> None:
        result = self.run_fixture("\n# cargo nextest run --archive-file stale.tar.zst\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("archive reuse via nextest", result.stderr)

    def test_rejects_delegated_nextest_archive_reuse(self) -> None:
        result = self.run_fixture(delegated_script="#!/bin/sh\ncargo nextest archive --archive-file stale.tar.zst\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("forbidden delegated archive reuse via nextest", result.stderr)

    def test_rejects_delegated_moon_task_execution(self) -> None:
        result = self.run_fixture(delegated_script="#!/bin/sh\nmoon run test\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("forbidden delegated task-result execution via Moon", result.stderr)

    def test_rejects_moon_task_execution(self) -> None:
        result = self.run_fixture("\n# moon run test\n")
        self.assertNotEqual(0, result.returncode)
        self.assertIn("task-result execution via Moon", result.stderr)

    def test_rejects_moon_configuration_until_provenance_closure_exists(self) -> None:
        result = self.run_fixture(moon=True)
        self.assertNotEqual(0, result.returncode)
        self.assertIn("task-result configuration through Moon is forbidden", result.stderr)


if __name__ == "__main__":
    unittest.main()
