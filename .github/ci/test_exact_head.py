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
ABI_WORKFLOW = SCRIPT.parents[1] / "workflows" / "abi-surface.yml"
NIGHTLY_WORKFLOW = SCRIPT.parents[1] / "workflows" / "nightly.yml"
HOOK = SCRIPT.parents[2] / ".githooks" / "pre-commit"
PYTHON_NATIVE_E2E_RUNNER = SCRIPT.parents[2] / "tools" / "run-python-native-e2e.sh"
PYTHON_GRPC_CHECK = SCRIPT.parents[2] / "tools" / "check-python-grpc.sh"


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

    def test_workflow_unions_trusted_and_proposed_classifier_lanes(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            'git show "$BASE_SHA:.github/ci/exact_head.py" > "$trusted_dir/exact_head.py"',
            workflow,
        )
        self.assertIn(
            '"$CI_PYTHON" "$TRUSTED_EXACT_HEAD_HELPER" classify --github-output "$trusted_output"',
            workflow,
        )
        self.assertIn(
            '"$CI_PYTHON" "$PROPOSED_EXACT_HEAD_HELPER" classify --github-output "$proposed_output"',
            workflow,
        )
        self.assertIn(
            'grep -qx "$lane=true" "$trusted_output" || grep -qx "$lane=true" "$proposed_output"',
            workflow,
        )
        self.assertIn(
            "The proposed\n          # classifier may add new lanes immediately, but cannot remove a lane",
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

    def test_workflow_consumes_generated_and_conformance_classifier_outputs(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("generated: ${{ steps.changes.outputs.generated }}", workflow)
        self.assertIn("conformance: ${{ steps.changes.outputs.conformance }}", workflow)
        self.assertIn("grpc_e2e: ${{ steps.changes.outputs.grpc_e2e }}", workflow)
        self.assertIn("if: needs.classify.outputs.generated == 'true'", workflow)
        self.assertIn("if: needs.classify.outputs.conformance == 'true'", workflow)
        self.assertIn('require_lane Generated "$GENERATED_REQUIRED" "$GENERATED_RESULT"', workflow)
        self.assertIn('require_lane Conformance "$CONFORMANCE_REQUIRED" "$CONFORMANCE_RESULT"', workflow)

    def test_abi_workflow_installs_rustfmt_for_binding_generation(self) -> None:
        workflow = ABI_WORKFLOW.read_text(encoding="utf-8")
        rust_step = workflow.split("- name: Install Rust toolchain", 1)[1].split(
            "- name: Verify self-hosted ABI dependencies", 1
        )[0]
        self.assertIn("components: rustfmt", rust_step)

    def test_nightly_contracts_installs_rustfmt_for_generation_check(self) -> None:
        workflow = NIGHTLY_WORKFLOW.read_text(encoding="utf-8")
        contracts = workflow.split("\n  contracts:\n", 1)[1].split("\n  client-e2e:\n", 1)[0]
        self.assertIn("uses: dtolnay/rust-toolchain@stable", contracts)
        self.assertIn("components: rustfmt", contracts)
        self.assertIn("bash tools/check-flatbuffers-generated.sh", contracts)

    def test_python_grpc_generation_check_uses_selected_runner_python(self) -> None:
        script = PYTHON_GRPC_CHECK.read_text(encoding="utf-8")
        self.assertIn('PYTHON_BIN="${CI_PYTHON:-python3}"', script)
        self.assertIn(
            '"$PYTHON_BIN" "$root/tools/check-python-dependencies.py" --codegen',
            script,
        )
        self.assertIn('"$PYTHON_BIN" -m grpc_tools.protoc', script)

    def test_typescript_behavior_installs_rust_for_semantic_snapshot(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        typescript = workflow.split("\n  typescript:\n", 1)[1].split(
            "\n  exact-head-gate:\n", 1
        )[0]
        self.assertIn("uses: dtolnay/rust-toolchain@stable", typescript)
        self.assertIn("./tools/test-typescript-generation.sh", typescript)
        self.assertIn("npm run check:generated --prefix clients/typescript", typescript)
        self.assertIn("npm run test:package --prefix clients/typescript", typescript)

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
                (repo / "tools").mkdir(parents=True)
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
                python_e2e = repo / "tools" / "run-python-native-e2e.sh"
                python_e2e.write_text(
                    "#!/usr/bin/env bash\n"
                    "printf 'python-native-e2e\\n' >> \"$HOOK_LOG\"\n",
                    encoding="utf-8",
                )
                python_e2e.chmod(0o755)

                git(repo, "add", ".githooks", ".github", "reference", "tools", "fakebin")
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

    def test_precommit_runs_python_native_e2e_only_for_grpc_sensitive_changes(self) -> None:
        cases = (
            ("core/native/vespa/abi.cc", True),
            ("core/crates/searchkernel-ir-adapter/src/backend.rs", True),
            ("core/crates/searchkernel/src/query.rs", False),
            ("searchkerneld/src/main.rs", True),
            ("core/crates/searchkernel/src/result.rs", False),
            ("docs/architecture.md", False),
        )
        for staged_path, expect_e2e in cases:
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
                (repo / "tools").mkdir(parents=True)
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
                python_e2e = repo / "tools" / "run-python-native-e2e.sh"
                python_e2e.write_text(
                    "#!/usr/bin/env bash\n"
                    "printf 'python-native-e2e\\n' >> \"$HOOK_LOG\"\n",
                    encoding="utf-8",
                )
                python_e2e.chmod(0o755)

                git(repo, "add", ".githooks", ".github", "reference", "tools", "fakebin")
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
                self.assertEqual("python-native-e2e" in logged, expect_e2e)

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
        (repo / "tools").mkdir(parents=True)
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
        python_e2e = repo / "tools" / "run-python-native-e2e.sh"
        python_e2e.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'python-native-e2e\\n' >> \"$HOOK_LOG\"\n",
            encoding="utf-8",
        )
        python_e2e.chmod(0o755)

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
        tracked = repo / "core" / "crates" / "searchkernel-ir" / "src" / "tracked.rs"
        tracked.parent.mkdir(parents=True, exist_ok=True)
        tracked.write_text("original\n", encoding="utf-8")

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
        tracked.write_text("unstaged\n", encoding="utf-8")

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
        logged = log.read_text(encoding="utf-8") if log.exists() else ""
        self.assertNotIn("cargo ", logged)
        self.assertNotIn("native i", logged)

    def test_python_native_e2e_runner_bootstraps_and_opts_in(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")

        (repo / "tools").mkdir(parents=True)
        (repo / "fakebin").mkdir(parents=True)
        (repo / "tools" / "run-python-native-e2e.sh").write_bytes(
            PYTHON_NATIVE_E2E_RUNNER.read_bytes()
        )

        log = repo / "runner.log"
        dev = repo / "dev"
        dev.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'dev %s\\n' \"$*\" >> \"$RUNNER_LOG\"\n",
            encoding="utf-8",
        )
        dev.chmod(0o755)
        fake_python = repo / "fakebin" / "python"
        fake_python.write_text(
            "#!/usr/bin/env bash\n"
            "printf 'opt_in=%s args=%s\\n' \"${SEARCHKERNEL_RUN_NATIVE_E2E:-}\" \"$*\" >> \"$RUNNER_LOG\"\n",
            encoding="utf-8",
        )
        fake_python.chmod(0o755)

        env = os.environ.copy()
        env["CI_PYTHON"] = str(fake_python)
        env["RUNNER_LOG"] = str(log)
        result = subprocess.run(
            ["bash", str(repo / "tools" / "run-python-native-e2e.sh")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        logged = log.read_text(encoding="utf-8")
        self.assertIn("dev native-sdk ensure", logged)
        self.assertIn("opt_in=1 args=-m unittest discover", logged)
        self.assertIn("-p test_grpc_e2e.py", logged)

    def test_classification_selects_behavioral_lanes_by_changed_area(self) -> None:
        cases = {
            b"core/crates/searchkernel-ir/src/model.rs\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=true",
                "native=true",
                "grpc_e2e=true",
                "selector=false",
            },
            b"clients/python/searchkernel/ergonomic.py\0": {
                "rust=false",
                "python=true",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=false",
                "grpc_e2e=true",
                "selector=false",
            },
            b"clients/typescript/src/index.ts\0": {
                "rust=false",
                "python=false",
                "typescript=true",
                "generated=false",
                "conformance=false",
                "native=false",
                "grpc_e2e=false",
                "selector=false",
            },
            b"core/native/vespa/abi.cc\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=true",
                "grpc_e2e=true",
                "selector=false",
            },
            b"searchkerneld/src/main.rs\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=true",
                "grpc_e2e=true",
                "selector=false",
            },
            b"api/searchkernel.fbs\0": {
                "rust=true",
                "python=true",
                "typescript=true",
                "generated=true",
                "conformance=true",
                "native=false",
                "grpc_e2e=true",
                "selector=false",
            },
            b"core/crates/searchkernel/src/query.rs\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=true",
                "grpc_e2e=false",
                "selector=false",
            },
            b"Cargo.lock\0": {
                "rust=true",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=true",
                "grpc_e2e=true",
                "selector=false",
            },
            b"dev\0": {
                "rust=true",
                "python=false",
                "typescript=true",
                "generated=false",
                "conformance=false",
                "native=false",
                "grpc_e2e=true",
                "selector=true",
            },
            b"docs/architecture.md\0": {
                "rust=false",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=false",
                "grpc_e2e=false",
                "selector=false",
            },
            b".githooks/pre-commit\0": {
                "rust=false",
                "python=false",
                "typescript=false",
                "generated=false",
                "conformance=false",
                "native=false",
                "grpc_e2e=false",
                "selector=true",
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


class PreCommitLaneTests(unittest.TestCase):
    def test_precommit_shell_syntax_is_valid(self) -> None:
        result = subprocess.run(
            ["bash", "-n", str(HOOK)],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout)

    def make_hook_repo(self) -> tuple[Path, tempfile.TemporaryDirectory[str], Path, dict[str, str]]:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")

        for directory in (
            ".githooks",
            ".github/ci",
            ".github/workflows",
            "reference/vespa",
            "reference/api-conformance",
            "tools",
            "fakebin",
            "core/crates/searchkernel-filter/src",
        ):
            (repo / directory).mkdir(parents=True, exist_ok=True)

        (repo / ".githooks/pre-commit").write_bytes(HOOK.read_bytes())
        (repo / ".github/ci/exact_head.py").write_bytes(SCRIPT.read_bytes())
        (repo / ".github/ci/test_exact_head.py").write_text("raise SystemExit('real selector test should be routed through CI_PYTHON')\n")
        (repo / ".github/workflows/behavioral-verification.yml").write_text("fixture\n")
        (repo / "core/crates/searchkernel-filter/src/lib.rs").write_text("original\n")

        log = repo / "hook.log"

        def executable(path: Path, body: str) -> None:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body, encoding="utf-8")
            path.chmod(0o755)

        executable(
            repo / "fakebin/cargo",
            'printf "cargo %s\\n" "$*" >> "$HOOK_LOG"\n',
        )
        executable(
            repo / "fakebin/fake-python",
            'printf "python %s\\n" "$*" >> "$HOOK_LOG"\n',
        )
        executable(
            repo / "dev",
            'printf "dev %s\\n" "$*" >> "$HOOK_LOG"\n',
        )
        executable(
            repo / "reference/vespa/verify-through.sh",
            'printf "native %s\\n" "$*" >> "$HOOK_LOG"\n',
        )
        executable(
            repo / "reference/api-conformance/verify.sh",
            'printf "conformance\\n" >> "$HOOK_LOG"\n',
        )
        executable(
            repo / "tools/run-python-native-e2e.sh",
            'printf "python-native-e2e\\n" >> "$HOOK_LOG"\n',
        )
        for name in (
            "check-flatbuffers-generated.sh",
            "check-flatbuffers-bfbs.sh",
            "check-searchkernel-rpc.sh",
            "check-python-grpc.sh",
        ):
            executable(
                repo / "tools" / name,
                f'printf "{name}\\n" >> "$HOOK_LOG"\n',
            )

        git(repo, "add", ".")
        git(repo, "commit", "-q", "-m", "fixture")

        env = os.environ.copy()
        env["HOOK_LOG"] = str(log)
        env["PATH"] = f"{repo / 'fakebin'}:{env['PATH']}"
        env["CI_PYTHON"] = str(repo / "fakebin/fake-python")
        return repo, temp, log, env

    def run_hook_for(self, staged_path: str) -> str:
        repo, _temp, log, env = self.make_hook_repo()
        staged = repo / staged_path
        staged.parent.mkdir(parents=True, exist_ok=True)
        staged.write_text("changed\n", encoding="utf-8")
        git(repo, "add", staged_path)
        result = subprocess.run(
            ["bash", str(repo / ".githooks/pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        return log.read_text(encoding="utf-8")

    def test_precommit_executes_python_typescript_generated_and_conformance_lanes(self) -> None:
        python_log = self.run_hook_for("clients/python/searchkernel/client.py")
        self.assertIn("python -m unittest discover -s clients/python/tests -p test_*.py", python_log)
        self.assertIn("python-native-e2e", python_log)
        self.assertNotIn("dev test typescript", python_log)

        typescript_log = self.run_hook_for("clients/typescript/src/index.ts")
        self.assertIn("dev test typescript", typescript_log)
        self.assertNotIn("python -m unittest discover", typescript_log)

        protocol_log = self.run_hook_for("api/rpc/searchkernel.proto")
        for expected in (
            "check-flatbuffers-generated.sh",
            "check-flatbuffers-bfbs.sh",
            "check-searchkernel-rpc.sh",
            "check-python-grpc.sh",
            "conformance",
            "python -m unittest discover -s clients/python/tests -p test_*.py",
            "dev test typescript",
            "python-native-e2e",
        ):
            self.assertIn(expected, protocol_log)

    def test_precommit_executes_selector_regressions_when_selector_changes(self) -> None:
        log = self.run_hook_for(".github/workflows/behavioral-verification.yml")
        self.assertIn("python .github/ci/test_exact_head.py", log)
        self.assertNotIn("native i", log)
        self.assertNotIn("python-native-e2e", log)

    def test_precommit_native_daemon_and_facade_paths_run_real_native_and_grpc_e2e(self) -> None:
        for path in (
            "searchkerneld/src/main.rs",
            "core/crates/searchkernel/src/index/native.rs",
            "core/crates/searchkernel/src/portable_definition.rs",
        ):
            with self.subTest(path=path):
                log = self.run_hook_for(path)
                self.assertIn("native i", log)
                self.assertIn("python-native-e2e", log)

        server_log = self.run_hook_for("searchkerneld/src/lib.rs")
        self.assertNotIn("native i", server_log)
        self.assertIn("python-native-e2e", server_log)

        rust_facade_log = self.run_hook_for("core/crates/searchkernel/src/query.rs")
        self.assertIn("native i", rust_facade_log)
        self.assertNotIn("python-native-e2e", rust_facade_log)

        shared_ir_log = self.run_hook_for("core/crates/searchkernel-ir/src/model.rs")
        self.assertIn("native i", shared_ir_log)
        self.assertIn("python-native-e2e", shared_ir_log)

    def test_precommit_trusted_classifier_prevents_same_commit_lane_weakening(self) -> None:
        repo, _temp, log, env = self.make_hook_repo()

        native = repo / "core/native/vespa/abi.cc"
        native.parent.mkdir(parents=True, exist_ok=True)
        native.write_text("changed\n", encoding="utf-8")
        git(repo, "add", "core/native/vespa/abi.cc")

        classifier = repo / ".github/ci/exact_head.py"
        classifier.write_text(
            "#!/usr/bin/env python3\n"
            "import argparse\n"
            "p=argparse.ArgumentParser(); p.add_argument('command'); "
            "p.add_argument('--github-output'); a=p.parse_args(); "
            "open(a.github_output,'w').write('rust=false\\npython=false\\ntypescript=false\\n"
            "generated=false\\nconformance=false\\nnative=false\\ngrpc_e2e=false\\nselector=false\\n')\n",
            encoding="utf-8",
        )
        git(repo, "add", ".github/ci/exact_head.py")

        result = subprocess.run(
            ["bash", str(repo / ".githooks/pre-commit")],
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
        self.assertIn("python-native-e2e", logged)
        self.assertIn("python .github/ci/test_exact_head.py", logged)

    def test_precommit_uses_staged_classifier_and_rejects_unstaged_classifier_edits(self) -> None:
        repo, _temp, log, env = self.make_hook_repo()
        hook_path = repo / ".githooks/pre-commit"
        hook_path.write_text(hook_path.read_text(encoding="utf-8") + "\n# staged selector change\n")
        git(repo, "add", ".githooks/pre-commit")

        # If the hook incorrectly executes this working-tree classifier, it
        # would emit no selected lanes and skip the commit-equivalence guard.
        classifier = repo / ".github/ci/exact_head.py"
        classifier.write_text(
            "#!/usr/bin/env python3\n"
            "import argparse\n"
            "p=argparse.ArgumentParser(); p.add_argument('command'); "
            "p.add_argument('--github-output'); a=p.parse_args(); "
            "open(a.github_output,'w').write('rust=false\\npython=false\\ntypescript=false\\n"
            "generated=false\\nconformance=false\\nnative=false\\ngrpc_e2e=false\\nselector=false\\n')\n",
            encoding="utf-8",
        )

        result = subprocess.run(
            ["bash", str(repo / ".githooks/pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tracked unstaged changes also exist", result.stdout)
        self.assertFalse(log.exists(), "verification must stop before running commands")

    def test_precommit_rejects_non_native_unstaged_relevant_changes(self) -> None:
        repo, _temp, log, env = self.make_hook_repo()
        staged = repo / "core/crates/searchkernel-ir/src/model.rs"
        staged.parent.mkdir(parents=True, exist_ok=True)
        staged.write_text("staged\n", encoding="utf-8")
        git(repo, "add", "core/crates/searchkernel-ir/src/model.rs")
        (repo / "core/crates/searchkernel-filter/src/lib.rs").write_text("unstaged\n")

        result = subprocess.run(
            ["bash", str(repo / ".githooks/pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tracked unstaged changes also exist", result.stdout)
        self.assertFalse(log.exists(), "verification must stop before running commands")

    def test_precommit_rejects_untracked_files_for_verified_commits(self) -> None:
        repo, _temp, log, env = self.make_hook_repo()
        staged = repo / "core/crates/searchkernel-ir/src/model.rs"
        staged.parent.mkdir(parents=True, exist_ok=True)
        staged.write_text("staged\n", encoding="utf-8")
        git(repo, "add", "core/crates/searchkernel-ir/src/model.rs")
        untracked = repo / "scratch-helper.txt"
        untracked.write_text("untracked\n", encoding="utf-8")

        result = subprocess.run(
            ["bash", str(repo / ".githooks/pre-commit")],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("untracked files also exist", result.stdout)
        self.assertFalse(log.exists(), "verification must stop before running commands")


if __name__ == "__main__":
    unittest.main()
