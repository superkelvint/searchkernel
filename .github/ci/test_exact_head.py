#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("exact_head.py")
WORKFLOW = SCRIPT.parents[1] / "workflows" / "behavioral-verification.yml"


def run(*args: str, cwd: Path | None = None, input_bytes: bytes | None = None) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=cwd,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=repo, text=True).strip()


class ExactHeadTests(unittest.TestCase):
    def make_repo(self) -> tuple[Path, tempfile.TemporaryDirectory[str]]:
        temp = tempfile.TemporaryDirectory()
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")
        (repo / "file.txt").write_text("one\n", encoding="utf-8")
        git(repo, "add", "file.txt")
        git(repo, "commit", "-q", "-m", "first")
        return repo, temp

    def test_previous_success_cannot_authorize_changed_head(self) -> None:
        repo, temp = self.make_repo()
        self.addCleanup(temp.cleanup)
        first = git(repo, "rev-parse", "HEAD")

        accepted = run("assert-head", "--expected", first, "--repo", str(repo))
        self.assertEqual(accepted.returncode, 0, accepted.stdout.decode())

        (repo / "file.txt").write_text("two\n", encoding="utf-8")
        git(repo, "add", "file.txt")
        git(repo, "commit", "-q", "-m", "second")
        second = git(repo, "rev-parse", "HEAD")
        self.assertNotEqual(first, second)

        stale = run("assert-head", "--expected", first, "--repo", str(repo))
        self.assertNotEqual(stale.returncode, 0)
        output = stale.stdout.decode()
        self.assertIn(first, output)
        self.assertIn(second, output)

        fresh = run("assert-head", "--expected", second, "--repo", str(repo))
        self.assertEqual(fresh.returncode, 0, fresh.stdout.decode())

    def test_base_helper_can_classify_pre_gate_head(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")
        (repo / "file.txt").write_text("root\n", encoding="utf-8")
        git(repo, "add", "file.txt")
        git(repo, "commit", "-q", "-m", "root")
        root = git(repo, "rev-parse", "HEAD")

        git(repo, "checkout", "-q", "-b", "base")
        helper_dir = repo / ".github" / "ci"
        helper_dir.mkdir(parents=True)
        (helper_dir / "exact_head.py").write_bytes(SCRIPT.read_bytes())
        git(repo, "add", ".github/ci/exact_head.py")
        git(repo, "commit", "-q", "-m", "add gate helper")
        base = git(repo, "rev-parse", "HEAD")

        git(repo, "checkout", "-q", "-b", "pre-gate-head", root)
        (repo / "file.txt").write_text("head change\n", encoding="utf-8")
        git(repo, "add", "file.txt")
        git(repo, "commit", "-q", "-m", "pre-gate change")
        self.assertFalse((repo / ".github" / "ci" / "exact_head.py").exists())

        fallback = repo / "base-exact-head.py"
        fallback.write_bytes(
            subprocess.check_output(
                ["git", "show", f"{base}:.github/ci/exact_head.py"],
                cwd=repo,
            )
        )
        classified = subprocess.run(
            [sys.executable, str(fallback), "classify"],
            cwd=repo,
            input=b"core/crates/searchkernel-ir/src/model.rs\0",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        self.assertEqual(classified.returncode, 0, classified.stdout.decode())
        self.assertIn("rust: required", classified.stdout.decode())

    def test_workflow_uses_base_classifier_for_authoritative_lane_selection(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            'git show "$BASE_SHA:.github/ci/exact_head.py" > "$trusted_dir/exact_head.py"',
            workflow,
        )
        self.assertIn(
            '"$CI_PYTHON" "$TRUSTED_EXACT_HEAD_HELPER" classify --github-output "$GITHUB_OUTPUT"',
            workflow,
        )
        self.assertNotIn(
            '"$CI_PYTHON" "$EXACT_HEAD_HELPER" classify --github-output "$GITHUB_OUTPUT"',
            workflow,
        )

    def test_classification_selects_behavioral_lanes_by_changed_area(self) -> None:
        cases = {
            b"core/crates/searchkernel-ir/src/model.rs\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "native=false",
            },
            b"clients/python/searchkernel/ergonomic.py\0": {
                "rust=false",
                "python=true",
                "typescript=false",
                "native=false",
            },
            b"clients/typescript/src/index.ts\0": {
                "rust=false",
                "python=false",
                "typescript=true",
                "native=false",
            },
            b"core/native/vespa/abi.cc\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "native=true",
            },
            b"api/searchkernel.fbs\0": {
                "rust=true",
                "python=true",
                "typescript=true",
                "native=false",
            },
            b"docs/architecture.md\0": {
                "rust=false",
                "python=false",
                "typescript=false",
                "native=false",
            },
        }
        for changed, expected in cases.items():
            with self.subTest(changed=changed):
                with tempfile.NamedTemporaryFile() as output_file:
                    result = run(
                        "classify",
                        "--github-output",
                        output_file.name,
                        input_bytes=changed,
                    )
                    self.assertEqual(result.returncode, 0, result.stdout.decode())
                    lines = {
                        line.strip()
                        for line in Path(output_file.name).read_text(encoding="utf-8").splitlines()
                        if line.strip()
                    }
                    self.assertEqual(lines, expected)


if __name__ == "__main__":
    unittest.main()
