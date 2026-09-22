#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("exact_head.py")
WORKFLOW = SCRIPT.parents[1] / "workflows" / "behavioral-verification.yml"
HOOK = SCRIPT.parents[2] / ".githooks" / "pre-commit"


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

    def test_workflow_keeps_native_signal_without_running_native_ci(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("\n  native:\n", workflow)
        self.assertNotIn("./dev native-sdk ensure", workflow)
        self.assertNotIn("./reference/vespa/verify-through.sh i", workflow)
        self.assertIn(
            'NATIVE_SENSITIVE: ${{ needs.classify.outputs.native }}',
            workflow,
        )
        self.assertIn(
            "Native-sensitive paths changed. CI intentionally does not bootstrap Vespa",
            workflow,
        )

    def test_precommit_runs_native_gate_only_for_native_sensitive_staged_paths(self) -> None:
        for staged_path, expect_native in (
            ("core/native/vespa/abi.cc", True),
            ("docs/architecture.md", False),
        ):
            with self.subTest(staged_path=staged_path):
                temp = tempfile.TemporaryDirectory()
                self.addCleanup(temp.cleanup)
                repo = Path(temp.name)
                git(repo, "init", "-q")
                git(repo, "config", "user.name", "CI Test")
                git(repo, "config", "user.email", "ci@example.invalid")

                (repo / ".githooks").mkdir(parents=True)
                (repo / ".github" / "ci").mkdir(parents=True)
                (repo / "reference" / "vespa").mkdir(parents=True)
                (repo / "fakebin").mkdir(parents=True)
                (repo / ".githooks" / "pre-commit").write_bytes(HOOK.read_bytes())
                (repo / ".github" / "ci" / "exact_head.py").write_bytes(SCRIPT.read_bytes())

                log = repo / "hook.log"
                cargo = repo / "fakebin" / "cargo"
                cargo.write_text(
                    "#!/usr/bin/env bash\n"
                    "printf 'cargo %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
                    encoding="utf-8",
                )
                cargo.chmod(0o755)
                native_gate = repo / "reference" / "vespa" / "verify-through.sh"
                native_gate.write_text(
                    "#!/usr/bin/env bash\n"
                    "printf 'native %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
                    encoding="utf-8",
                )
                native_gate.chmod(0o755)

                git(repo, "add", ".githooks", ".github", "reference", "fakebin")
                git(repo, "commit", "-q", "-m", "fixture")

                staged = repo / staged_path
                staged.parent.mkdir(parents=True, exist_ok=True)
                staged.write_text("changed\n", encoding="utf-8")
                git(repo, "add", staged_path)

                env = os.environ.copy()
                env["HOOK_LOG"] = str(log)
                env["PATH"] = f"{repo / 'fakebin'}:{env['PATH']}"
                result = subprocess.run(
                    ["bash", str(repo / ".githooks" / "pre-commit")],
                    cwd=repo,
                    env=env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    check=False,
                    text=True,
                )
                self.assertEqual(result.returncode, 0, result.stdout)
                logged = log.read_text(encoding="utf-8")
                self.assertIn("cargo fmt --all --check", logged)
                self.assertIn(
                    "cargo clippy --workspace --all-targets --no-default-features -- -D warnings",
                    logged,
                )
                self.assertIn(
                    "cargo test --workspace --all-targets --no-default-features",
                    logged,
                )
                self.assertEqual("native i" in logged, expect_native)

    def test_precommit_treats_native_rename_as_native_sensitive(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")

        (repo / ".githooks").mkdir(parents=True)
        (repo / ".github" / "ci").mkdir(parents=True)
        (repo / "reference" / "vespa").mkdir(parents=True)
        (repo / "fakebin").mkdir(parents=True)
        (repo / ".githooks" / "pre-commit").write_bytes(HOOK.read_bytes())
        (repo / ".github" / "ci" / "exact_head.py").write_bytes(SCRIPT.read_bytes())

        log = repo / "hook.log"
        cargo = repo / "fakebin" / "cargo"
        cargo.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'cargo %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
            encoding="utf-8",
        )
        cargo.chmod(0o755)
        native_gate = repo / "reference" / "vespa" / "verify-through.sh"
        native_gate.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'native %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
            encoding="utf-8",
        )
        native_gate.chmod(0o755)

        source = repo / "core" / "native" / "vespa" / "moved.cc"
        source.parent.mkdir(parents=True)
        source.write_text("native\n", encoding="utf-8")
        git(repo, "add", ".")
        git(repo, "commit", "-q", "-m", "fixture")

        destination = repo / "docs" / "moved.cc"
        destination.parent.mkdir(parents=True)
        source.rename(destination)
        git(repo, "add", "-A")

        env = os.environ.copy()
        env["HOOK_LOG"] = str(log)
        env["PATH"] = f"{repo / 'fakebin'}:{env['PATH']}"
        result = subprocess.run(
            ["bash", str(repo / ".githooks" / "pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        logged = log.read_text(encoding="utf-8")
        self.assertIn("native i", logged)

    def test_precommit_rejects_native_verification_with_unstaged_tracked_changes(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")

        (repo / ".githooks").mkdir(parents=True)
        (repo / ".github" / "ci").mkdir(parents=True)
        (repo / "reference" / "vespa").mkdir(parents=True)
        (repo / "fakebin").mkdir(parents=True)
        (repo / ".githooks" / "pre-commit").write_bytes(HOOK.read_bytes())
        (repo / ".github" / "ci" / "exact_head.py").write_bytes(SCRIPT.read_bytes())
        (repo / "tracked.txt").write_text("original\n", encoding="utf-8")

        log = repo / "hook.log"
        cargo = repo / "fakebin" / "cargo"
        cargo.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'cargo %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
            encoding="utf-8",
        )
        cargo.chmod(0o755)
        native_gate = repo / "reference" / "vespa" / "verify-through.sh"
        native_gate.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'native %s\\n' \"$*\" >> \"$HOOK_LOG\"\n",
            encoding="utf-8",
        )
        native_gate.chmod(0o755)

        git(repo, "add", ".")
        git(repo, "commit", "-q", "-m", "fixture")

        native = repo / "core" / "native" / "vespa" / "abi.cc"
        native.parent.mkdir(parents=True)
        native.write_text("changed\n", encoding="utf-8")
        git(repo, "add", "core/native/vespa/abi.cc")
        (repo / "tracked.txt").write_text("unstaged\n", encoding="utf-8")

        env = os.environ.copy()
        env["HOOK_LOG"] = str(log)
        env["PATH"] = f"{repo / 'fakebin'}:{env['PATH']}"
        result = subprocess.run(
            ["bash", str(repo / ".githooks" / "pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tracked unstaged changes also exist", result.stdout)
        logged = log.read_text(encoding="utf-8")
        self.assertNotIn("native i", logged)

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
