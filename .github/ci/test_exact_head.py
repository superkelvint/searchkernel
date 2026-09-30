#!/usr/bin/env python3
from __future__ import annotations

import ast
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("exact_head.py")
WORKFLOW = SCRIPT.parents[1] / "workflows" / "behavioral-verification.yml"
ABI_WORKFLOW = SCRIPT.parents[1] / "workflows" / "abi-surface.yml"
RUSTFMT_AUTOFIX_WORKFLOW = SCRIPT.parents[1] / "workflows" / "rustfmt-autofix.yml"
CI_IMAGE_RUNNER_WORKFLOW = SCRIPT.parent / "ci-image-runner.yml"
CLIENT_CAPABILITIES_WORKFLOW = SCRIPT.parents[1] / "workflows" / "client-capabilities.yml"
CLIENT_CAPABILITIES_AUTOFIX_PREPARE = SCRIPT.parents[1] / "workflows" / "client-capabilities-autofix-prepare.yml"
CLIENT_CAPABILITIES_AUTOFIX_APPLY = SCRIPT.parents[1] / "workflows" / "client-capabilities-autofix-apply.yml"
PYTHON_NATIVE_E2E_RUNNER = SCRIPT.parents[2] / "tools" / "run-python-native-e2e.sh"
PERSISTENCE_IMMUTABILITY = SCRIPT.parents[2] / "tools" / "check-persistence-fixture-immutability.sh"
DEV = SCRIPT.parents[2] / "dev"
VERIFICATION_DAG_MODULE = SCRIPT.parents[2] / "verification" / "dag.py"
VERIFICATION_DAG_FILE = SCRIPT.parents[2] / "verification" / "dag.toml"
CONSUMER_RELEASE_PACKAGER = SCRIPT.parents[2] / "tools" / "package-consumer-release.py"


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


def write_verification_policy(repo: Path) -> None:
    (repo / ".github" / "ci").mkdir(parents=True, exist_ok=True)
    (repo / ".github" / "ci" / "exact_head.py").write_bytes(SCRIPT.read_bytes())
    (repo / "verification").mkdir(parents=True, exist_ok=True)
    (repo / "verification" / "dag.py").write_bytes(VERIFICATION_DAG_MODULE.read_bytes())
    (repo / "verification" / "dag.toml").write_bytes(VERIFICATION_DAG_FILE.read_bytes())


def rust_bundle_paths() -> list[str]:
    tree = ast.parse(CONSUMER_RELEASE_PACKAGER.read_text(encoding="utf-8"))
    for node in tree.body:
        if not isinstance(node, ast.Assign):
            continue
        if not any(isinstance(target, ast.Name) and target.id == "RUST_BUNDLE_PATHS" for target in node.targets):
            continue
        value = ast.literal_eval(node.value)
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
            raise AssertionError("RUST_BUNDLE_PATHS must be a literal list of strings")
        return value
    raise AssertionError("RUST_BUNDLE_PATHS assignment not found")


def classify_lines(*paths: str) -> set[str]:
    with tempfile.NamedTemporaryFile() as output_file:
        result = run(
            "classify",
            "--github-output",
            output_file.name,
            input_bytes=b"\0".join(path.encode() for path in paths) + b"\0",
        )
        if result.returncode != 0:
            raise AssertionError(result.stdout.decode())
        return set(Path(output_file.name).read_text(encoding="utf-8").splitlines())


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
        verification_dir = repo / "verification"
        verification_dir.mkdir(parents=True)
        (verification_dir / "dag.py").write_bytes(VERIFICATION_DAG_MODULE.read_bytes())
        (verification_dir / "dag.toml").write_bytes(VERIFICATION_DAG_FILE.read_bytes())
        git(repo, "add", ".github/ci/exact_head.py", "verification/dag.py", "verification/dag.toml")
        git(repo, "commit", "-q", "-m", "add trusted verification policy")
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
        trusted_dag_module = repo / "base-dag.py"
        trusted_dag_module.write_bytes(
            subprocess.check_output(
                ["git", "show", f"{base}:verification/dag.py"],
                cwd=repo,
            )
        )
        trusted_dag_file = repo / "base-dag.toml"
        trusted_dag_file.write_bytes(
            subprocess.check_output(
                ["git", "show", f"{base}:verification/dag.toml"],
                cwd=repo,
            )
        )
        env = os.environ.copy()
        env["SEARCHKERNEL_VERIFICATION_DAG_MODULE"] = str(trusted_dag_module)
        env["SEARCHKERNEL_VERIFICATION_DAG"] = str(trusted_dag_file)
        classified = subprocess.run(
            [sys.executable, str(fallback), "classify"],
            cwd=repo,
            env=env,
            input=b"core/crates/searchkernel-ir/src/model.rs\0",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        self.assertEqual(classified.returncode, 0, classified.stdout.decode())
        self.assertIn("rust: required", classified.stdout.decode())

    def test_rustfmt_autofix_workflow_preserves_write_safety_contract(self) -> None:
        workflow = RUSTFMT_AUTOFIX_WORKFLOW.read_text(encoding="utf-8")
        runner = CI_IMAGE_RUNNER_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "if: github.event.pull_request.head.repo.full_name == github.repository",
            workflow,
        )
        self.assertIn("uses: ./.github/ci/ci-image-runner.yml", workflow)
        self.assertIn("revision: ${{ github.event.pull_request.head.sha }}", workflow)
        self.assertIn("target: rustfmt-autofix", workflow)
        self.assertIn("head_ref: ${{ github.event.pull_request.head.ref }}", workflow)
        self.assertIn(
            "autofix_token: ${{ secrets.RUSTFMT_AUTOFIX_TOKEN }}",
            workflow,
        )

        checkout = runner.split("- name: Check out requested revision", 1)[1].split(
            "- name: Resolve exact revision", 1
        )[0]
        self.assertIn("ref: ${{ inputs.revision || github.sha }}", checkout)
        self.assertIn("persist-credentials: false", checkout)

        rustfmt = runner.split("            rustfmt-autofix)", 1)[1].split(
            "            *)", 1
        )[0]
        self.assertIn("run_phase rustfmt cargo fmt --all", rustfmt)
        self.assertIn("grep -Ev '\\.rs$' || true", rustfmt)
        self.assertIn('if [[ -n "$unexpected" ]]; then', rustfmt)
        self.assertIn("exit 1", rustfmt)
        self.assertIn('[[ -n "${AUTOFIX_TOKEN:-}" ]] || {', rustfmt)
        self.assertIn('[[ -n "${HEAD_REF:-}" ]] || {', rustfmt)
        self.assertIn("assert_current_pr_head rustfmt-push", rustfmt)
        self.assertLess(
            rustfmt.index("assert_current_pr_head rustfmt-push"),
            rustfmt.index("git add -u"),
        )
        self.assertLess(
            rustfmt.index("assert_current_pr_head rustfmt-push"),
            rustfmt.index("git push"),
        )
        self.assertIn("git add -u", rustfmt)
        self.assertIn('"HEAD:refs/heads/${HEAD_REF}"', rustfmt)

    def test_candidate_regressions_use_candidate_verification_policy(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        regression_step = workflow.split("- name: Verify exact-head CI regressions", 1)[1].split(
            "- name: Verify candidate verification DAG integrity and coverage", 1
        )[0]
        self.assertIn(
            "SEARCHKERNEL_VERIFICATION_DAG_MODULE: ${{ github.workspace }}/verification/dag.py",
            regression_step,
        )
        self.assertIn(
            "SEARCHKERNEL_VERIFICATION_DAG: ${{ github.workspace }}/verification/dag.toml",
            regression_step,
        )

    def test_workflow_uses_base_classifier_for_authoritative_lane_selection(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            'git show "$BASE_SHA:.github/ci/exact_head.py" > "$trusted_dir/exact_head.py"',
            workflow,
        )
        self.assertIn(
            'git show "$BASE_SHA:verification/dag.py" > "$trusted_dir/dag.py"',
            workflow,
        )
        self.assertIn(
            'git show "$BASE_SHA:verification/dag.toml" > "$trusted_dir/dag.toml"',
            workflow,
        )
        self.assertIn(
            'SEARCHKERNEL_VERIFICATION_DAG_MODULE=$trusted_dir/dag.py',
            workflow,
        )
        self.assertIn(
            'SEARCHKERNEL_VERIFICATION_DAG=$trusted_dir/dag.toml',
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

    def test_client_capabilities_routes_native_e2e_through_alma(self) -> None:
        workflow = CLIENT_CAPABILITIES_WORKFLOW.read_text(encoding="utf-8")
        verify = workflow.split("\n  verify:\n", 1)[1].split("\n  e2e:\n", 1)[0]
        e2e = workflow.split("\n  e2e:\n", 1)[1]
        self.assertIn("uses: dtolnay/rust-toolchain@stable", verify)
        self.assertIn(
            "uses: superkelvint/searchkernel/.github/workflows/native-verifier-runner.yml@main",
            e2e,
        )
        self.assertIn(
            'revision: ${{ github.event.pull_request.head.sha || github.sha }}',
            e2e,
        )
        self.assertIn("target: client-e2e", e2e)
        self.assertNotIn("Install newest immutable native Vespa base", workflow)
        self.assertNotIn("sudo apt-get", e2e)

    def test_client_capabilities_watches_release_gate_inputs(self) -> None:
        workflow = CLIENT_CAPABILITIES_WORKFLOW.read_text(encoding="utf-8")
        root = SCRIPT.parents[2]
        expected = []
        for relative in rust_bundle_paths():
            expected.append(f"{relative}/**" if (root / relative).is_dir() else relative)
        expected.extend(
            (
                "docs/release-support.json",
                "docs/release-support.md",
                "tools/check-release-support.py",
                "tools/package-consumer-release.py",
                "tools/check-consumer-release.py",
                "tools/test_package_consumer_release.py",
            )
        )
        for relative in expected:
            with self.subTest(relative=relative):
                self.assertEqual(
                    workflow.count(f'      - "{relative}"'),
                    2,
                    f"{relative} must trigger both pull_request and main push release qualification",
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

    def test_abi_workflow_installs_rustfmt_for_binding_generation(self) -> None:
        workflow = ABI_WORKFLOW.read_text(encoding="utf-8")
        rust_step = workflow.split("- name: Install Rust toolchain", 1)[1].split(
            "- name: Verify self-hosted ABI dependencies", 1
        )[0]
        self.assertIn("components: rustfmt", rust_step)

    def test_dev_abi_verifier_uses_repository_header_without_sdk_dependency(self) -> None:
        source = DEV.read_text(encoding="utf-8")
        self.assertIn(
            'SK_NATIVE_ABI_HEADER="$ROOT/core/native/vespa/searchkernel_vespa.h" \\\n'
            '                bash "$ROOT/tools/check-native-bindings.sh"',
            source,
        )

    def test_client_capability_autofix_prepare_is_exact_head_and_generated_only(self) -> None:
        workflow = CLIENT_CAPABILITIES_AUTOFIX_PREPARE.read_text(encoding="utf-8")

        self.assertIn("\n  pull_request:\n", workflow)
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn(
            "ref: ${{ github.event.pull_request.head.sha }}",
            workflow,
        )
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn('"$CI_PYTHON" tools/check-capabilities.py --write', workflow)
        self.assertIn('"$CI_PYTHON" tools/check-client-contracts.py --write', workflow)
        self.assertIn('"$CI_PYTHON" tools/check-capabilities.py --check', workflow)
        self.assertIn('"$CI_PYTHON" tools/check-client-contracts.py --check', workflow)
        self.assertIn(
            "allowed_regex='^(LIMITATIONS\\.md|docs/CAPABILITIES\\.md|docs/CLIENT_CAPABILITIES\\.md|docs/client-capabilities\\.json)$'",
            workflow,
        )
        self.assertIn("git ls-files --others --exclude-standard", workflow)
        self.assertIn('summary="$(git diff --summary)"', workflow)
        self.assertIn("git diff --binary -- LIMITATIONS.md", workflow)
        self.assertIn("if (( bytes > 5242880 )); then", workflow)
        self.assertIn("retention-days: 1", workflow)


    def test_client_capability_autofix_apply_rejects_stale_or_unsafe_pushes(self) -> None:
        workflow = CLIENT_CAPABILITIES_AUTOFIX_APPLY.read_text(encoding="utf-8")

        self.assertIn("\n  workflow_run:\n", workflow)
        self.assertIn('workflows: ["Client capabilities autofix prepare"]', workflow)
        self.assertIn("permissions:\n  actions: read\n  contents: read", workflow)
        self.assertIn("github.event.workflow_run.conclusion == 'success'", workflow)
        self.assertIn("github.event.workflow_run.event == 'pull_request'", workflow)
        self.assertIn(
            "github.event.workflow_run.head_repository.full_name == github.repository",
            workflow,
        )
        self.assertIn("ref: ${{ github.event.workflow_run.head_sha }}", workflow)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("run-id: ${{ github.event.workflow_run.id }}", workflow)
        self.assertIn("git apply --check --whitespace=error-all", workflow)
        self.assertIn(
            "allowed_regex='^(LIMITATIONS\\.md|docs/CAPABILITIES\\.md|docs/CLIENT_CAPABILITIES\\.md|docs/client-capabilities\\.json)$'",
            workflow,
        )
        self.assertIn('summary="$(git diff --summary)"', workflow)
        self.assertIn(
            'git ls-remote "https://x-access-token:${READ_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"',
            workflow,
        )
        self.assertIn('if [[ "${remote_head,,}" != "${EXPECTED_HEAD,,}" ]]; then', workflow)
        self.assertIn("STALE_BRANCH=true", workflow)
        self.assertIn("AUTOFIX_TOKEN: ${{ secrets.RUSTFMT_AUTOFIX_TOKEN }}", workflow)
        self.assertIn('"HEAD:refs/heads/${HEAD_REF}"', workflow)
        self.assertNotIn('"HEAD:${HEAD_REF}"', workflow)


    def test_runner_python_selection_never_uses_bare_system_python(self) -> None:
        root = SCRIPT.parents[2]
        selector = (root / ".github" / "ci" / "select-python.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("sys.prefix != sys.base_prefix", selector)
        self.assertIn('-m venv "$venv_dir"', selector)
        self.assertIn("CI_PYTHON=$selected", selector)
        self.assertIn("python3-venv", selector)
        self.assertIn("sudo -n apt-get", selector)

    def test_typescript_behavior_installs_rust_for_semantic_snapshot(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        typescript = workflow.split("\n  typescript:\n", 1)[1].split(
            "\n  ruby:\n", 1
        )[0]
        self.assertIn("uses: dtolnay/rust-toolchain@stable", typescript)
        self.assertIn("./tools/test-typescript-generation.sh", typescript)
        self.assertIn("npm run check:generated --prefix clients/typescript", typescript)
        self.assertIn("npm run test:package --prefix clients/typescript", typescript)

    def test_ruby_behavior_is_exact_head_gated(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        ruby = workflow.split("\n  ruby:\n", 1)[1].split(
            "\n  exact-head-gate:\n", 1
        )[0]
        self.assertIn("if: needs.classify.outputs.ruby == 'true'", ruby)
        self.assertNotIn("uses: ruby/setup-ruby@v1", ruby)
        self.assertIn("Ensure provisioned Ruby", ruby)
        self.assertIn("sudo -n apt-get install -y --no-install-recommends ruby-full", ruby)
        self.assertIn("Gem::Version.new(RUBY_VERSION)", ruby)
        self.assertIn("uses: dtolnay/rust-toolchain@stable", ruby)
        self.assertIn("./dev install flatbuffers", ruby)
        self.assertIn("ruby --version", ruby)
        self.assertIn("./dev test ruby", ruby)
        classify = workflow.split("\n  classify:\n", 1)[1].split("\n  rust:\n", 1)[0]
        self.assertNotIn("uses: ruby/setup-ruby@v1", classify)
        provisioned_probe = ruby.split("- name: Ensure provisioned Ruby", 1)[1].split(
            "- name: Install Rust toolchain", 1
        )[0]
        self.assertIn("hashFiles('clients/ruby/**')", provisioned_probe)
        gate = workflow.split("\n  exact-head-gate:\n", 1)[1]
        self.assertIn("RUBY_REQUIRED: ${{ needs.classify.outputs.ruby }}", gate)
        self.assertIn("RUBY_RESULT: ${{ needs.ruby.result }}", gate)
        self.assertIn('require_lane Ruby "$RUBY_REQUIRED" "$RUBY_RESULT"', gate)

    def test_superseded_workflow_does_not_strand_invariant_gate(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        gate = workflow.split("\n  exact-head-gate:\n", 1)[1]
        self.assertIn("if: ${{ always() && !cancelled() }}", gate)

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
                "rust=true", "python=false", "typescript=false", "php=false", "ruby=false", "native=false",
                "protocol=false", "api_conformance=false", "contracts=false", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"clients/python/searchkernel/ergonomic.py\0": {
                "rust=false", "python=true", "typescript=false", "php=false", "ruby=false", "native=false",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"clients/typescript/src/index.ts\0": {
                "rust=false", "python=false", "typescript=true", "php=false", "ruby=false", "native=false",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"clients/ruby/lib/searchkernel.rb\0": {
                "rust=false", "python=false", "typescript=false", "php=false", "ruby=true", "native=false",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"core/native/vespa/abi.cc\0": {
                "rust=true", "python=false", "typescript=false", "php=false", "ruby=false", "native=true",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=true",
                "persistence=true", "issue_queue=false", "verification=false",
            },
            b"api/searchkernel.fbs\0": {
                "rust=true", "python=true", "typescript=true", "php=true", "ruby=true", "native=true",
                "protocol=true", "api_conformance=true", "contracts=true", "abi=false",
                "persistence=true", "issue_queue=false", "verification=false",
            },
            b"api/rpc/searchkernel.proto\0": {
                "rust=true", "python=true", "typescript=true", "php=true", "ruby=false", "native=false",
                "protocol=true", "api_conformance=false", "contracts=true", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"tools/copy-shared-library-into-sdk.sh\0": {
                "rust=true", "python=false", "typescript=false", "php=false", "ruby=false", "native=true",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=true",
                "persistence=true", "issue_queue=false", "verification=false",
            },
            b"reference/api-conformance/golden/example.json\0": {
                "rust=false", "python=false", "typescript=false", "php=false", "ruby=false", "native=false",
                "protocol=false", "api_conformance=true", "contracts=false", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
            },
            b"tools/check-native-abi-authorities.py\0": {
                "rust=true", "python=false", "typescript=false", "php=false", "ruby=false", "native=true",
                "protocol=false", "api_conformance=false", "contracts=true", "abi=true",
                "persistence=true", "issue_queue=false", "verification=false",
            },
            b"docs/architecture.md\0": {
                "rust=false", "python=false", "typescript=false", "php=false", "ruby=false", "native=false",
                "protocol=false", "api_conformance=false", "contracts=false", "abi=false",
                "persistence=false", "issue_queue=false", "verification=false",
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


    def assert_unclassified_path_blocks(self, path: str) -> None:
        for command in ("classify", "plan"):
            with self.subTest(path=path, command=command):
                args = (command, "--json") if command == "plan" else (command,)
                result = run(*args, input_bytes=path.encode() + b"\0")
                self.assertEqual(result.returncode, 3, result.stdout.decode())
                output = result.stdout.decode()
                self.assertIn("Unclassified behavioral paths", output)
                self.assertIn(path, output)

    def test_unknown_behavioral_path_blocks_authorization(self) -> None:
        self.assert_unclassified_path_blocks("new-runtime/src/engine.rs")

    def test_new_core_crate_blocks(self) -> None:
        self.assert_unclassified_path_blocks("core/crates/searchkernel-new/src/lib.rs")

    def test_registered_ruby_client_family_selects_ruby_contract_verification(self) -> None:
        lines = classify_lines("clients/ruby/lib/searchkernel.rb")
        self.assertIn("ruby=true", lines)
        self.assertIn("contracts=true", lines)
        for lane in ("rust", "python", "typescript", "php", "native", "protocol"):
            self.assertIn(f"{lane}=false", lines)

    def test_unregistered_client_family_still_blocks(self) -> None:
        self.assert_unclassified_path_blocks("clients/go/searchkernel.go")

    def test_new_api_contract_family_blocks(self) -> None:
        self.assert_unclassified_path_blocks("api/new_contract.fbs")
        self.assert_unclassified_path_blocks("api/rpc/new_transport.proto")

    def test_new_vespa_verifier_family_blocks(self) -> None:
        self.assert_unclassified_path_blocks("reference/vespa/m15/verify-m15.sh")

    def test_explicit_non_behavioral_paths_do_not_select_behavioral_lanes(self) -> None:
        for path in (
            "docs/architecture.md",
            "nested/README.md",
            "core/README.md",
            "core/native/README.md",
        ):
            with self.subTest(path=path):
                lines = classify_lines(path)
                self.assertTrue(lines)
                self.assertFalse(
                    [line for line in lines if line.endswith("=true")],
                    lines,
                )

    def test_tracked_scratch_like_file_blocks(self) -> None:
        self.assert_unclassified_path_blocks("notes.tmp")

    def test_behavioral_documentation_overrides_non_behavioral_exemption(self) -> None:
        lines = classify_lines("docs/CAPABILITIES.md")
        self.assertIn("contracts=true", lines)
        self.assertTrue(
            any(line.endswith("=true") for line in lines),
            lines,
        )

    def test_non_markdown_file_under_docs_blocks_when_unclassified(self) -> None:
        self.assert_unclassified_path_blocks("docs/new-runtime-policy.json")

    def test_schema_changes_fan_out_across_protocol_clients_contracts_and_persistence(self) -> None:
        lines = classify_lines("api/searchkernel.fbs")
        for lane in (
            "rust",
            "python",
            "typescript",
            "php",
            "ruby",
            "native",
            "protocol",
            "api_conformance",
            "contracts",
            "persistence",
        ):
            self.assertIn(f"{lane}=true", lines)

    def test_generated_artifact_changes_fan_out_to_cross_language_protocol_checks(self) -> None:
        lines = classify_lines("clients/python/searchkernel/_wire_generated/SearchKernel/V2/Foo.py")
        for lane in ("rust", "python", "typescript", "protocol", "contracts"):
            self.assertIn(f"{lane}=true", lines)

    def test_native_and_abi_changes_fan_out_to_persistence_and_contracts(self) -> None:
        lines = classify_lines("core/native/vespa/abi.cc")
        for lane in ("rust", "native", "abi", "persistence", "contracts"):
            self.assertIn(f"{lane}=true", lines)

    def test_persistence_storage_changes_select_native_persistence(self) -> None:
        lines = classify_lines("reference/persistence/fixtures/example/METADATA.json")
        for lane in ("rust", "native", "persistence"):
            self.assertIn(f"{lane}=true", lines)

    def test_root_build_metadata_selects_all_current_lanes(self) -> None:
        lines = classify_lines("Cargo.lock")
        self.assertFalse(
            [line for line in lines if line.endswith("=false")],
            lines,
        )

    def test_verification_orchestration_changes_select_all_current_lanes(self) -> None:
        lines = classify_lines(".github/workflows/behavioral-verification.yml")
        self.assertFalse(
            [line for line in lines if line.endswith("=false")],
            lines,
        )

    def test_ci_diff_disables_rename_detection_so_old_and_new_paths_are_classified(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            'git diff --name-only --no-renames -z "$BASE_SHA" "$HEAD_SHA" -- \\',
            workflow,
        )

        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")

        source = repo / "clients" / "python" / "searchkernel" / "moved.py"
        source.parent.mkdir(parents=True)
        source.write_text("old\n", encoding="utf-8")
        git(repo, "add", ".")
        git(repo, "commit", "-q", "-m", "base")
        base = git(repo, "rev-parse", "HEAD")

        destination = repo / "clients" / "typescript" / "src" / "moved.ts"
        destination.parent.mkdir(parents=True)
        source.rename(destination)
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "-m", "move")
        head = git(repo, "rev-parse", "HEAD")

        changed = subprocess.check_output(
            ["git", "diff", "--name-only", "--no-renames", "-z", base, head, "--"],
            cwd=repo,
        )
        result = run("classify", input_bytes=changed)
        self.assertEqual(result.returncode, 0, result.stdout.decode())
        output = result.stdout.decode()
        self.assertIn("python: required", output)
        self.assertIn("typescript: required", output)

    def test_trusted_plan_emits_stable_task_ids_with_reasons(self) -> None:
        result = run(
            "plan",
            "--json",
            input_bytes=b"core/native/vespa/abi.cc\0",
        )
        self.assertEqual(result.returncode, 0, result.stdout.decode())
        payload = json.loads(result.stdout.decode())
        tasks = payload["tasks"]
        self.assertEqual(
            [task["id"] for task in tasks],
            [
                "rust-non-native",
                "rust-quality",
                "client-contracts",
                "portable-ir-boundary",
                "abi",
                "native-full",
                "milestone.m14",
                "vespa.nearest-neighbor.hnsw",
                "product.integration",
                "product.rust-real-engine",
                "vespa.grouping.yql",
                "vespa.recovery",
                "native-additive-regression",
                "native-python-e2e",
                "persistence-native",
                "persistence-static",
            ],
        )
        for task in tasks:
            self.assertTrue(task["reasons"], task)
            self.assertTrue(
                any("core/native/vespa/abi.cc" in reason for reason in task["reasons"]),
                task,
            )

    def test_trusted_plan_is_deterministic_across_input_order_and_duplicates(self) -> None:
        forward = run(
            "plan",
            "--json",
            input_bytes=(
                b"core/native/vespa/abi.cc\0"
                b"api/searchkernel.fbs\0"
                b"core/native/vespa/abi.cc\0"
            ),
        )
        reverse = run(
            "plan",
            "--json",
            input_bytes=(
                b"api/searchkernel.fbs\0"
                b"core/native/vespa/abi.cc\0"
            ),
        )
        self.assertEqual(forward.returncode, 0, forward.stdout.decode())
        self.assertEqual(reverse.returncode, 0, reverse.stdout.decode())
        self.assertEqual(forward.stdout, reverse.stdout)

    def test_candidate_moon_configuration_cannot_reduce_trusted_task_set(self) -> None:
        result = run(
            "plan",
            "--json",
            input_bytes=b".moon/workspace.yml\0",
        )
        self.assertEqual(result.returncode, 0, result.stdout.decode())
        payload = json.loads(result.stdout.decode())
        task_ids = {task["id"] for task in payload["tasks"]}
        for task_id in (
            "full",
            "portable-full",
            "clients-full",
            "native-full",
            "web-dashboard",
            "web-full",
            "milestone.m14",
            "vespa.recovery",
            "native-additive-regression",
            "portable-ir-boundary",
            "verification-policy",
            "frontend-harness-selftest",
            "frontend-verifier-integrity",
        ):
            self.assertIn(task_id, task_ids)
        for task in payload["tasks"]:
            self.assertTrue(
                any(".moon/workspace.yml" in reason for reason in task["reasons"]),
                task,
            )

    def test_classification_covers_verification_surfaces(self) -> None:
        cases = {
            b"searchkerneld/src/main.rs\0": {"native=true"},
            b"core/crates/searchkernel/src/index/native.rs\0": {"native=true"},
            b"core/crates/searchkernel/src/index/manifest.rs\0": {"native=true"},
            b"core/crates/searchkernel/src/portable_definition.rs\0": {"native=true"},
            b"api/rpc/searchkernel.proto\0": {"protocol=true", "persistence=false", "contracts=true"},
            b"tools/copy-shared-library-into-sdk.sh\0": {"native=true", "abi=true"},
            b"api/searchkernel.fbs\0": {"protocol=true"},
            b"reference/api-conformance/golden/example.json\0": {"api_conformance=true"},
            b".github/ci/exact_head.py\0": {"verification=true"},
            b".github/ci/test_exact_head.py\0": {"verification=true"},
        }
        for changed, required in cases.items():
            with self.subTest(changed=changed):
                with tempfile.NamedTemporaryFile() as output_file:
                    result = run(
                        "classify",
                        "--github-output",
                        output_file.name,
                        input_bytes=changed,
                    )
                    self.assertEqual(result.returncode, 0, result.stdout.decode())
                    lines = set(
                        Path(output_file.name).read_text(encoding="utf-8").splitlines()
                    )
                    self.assertTrue(required <= lines, (changed, required, lines))

    def test_persistence_immutability_can_check_the_staged_snapshot(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        repo = Path(temp.name)
        git(repo, "init", "-q")
        git(repo, "config", "user.name", "CI Test")
        git(repo, "config", "user.email", "ci@example.invalid")
        fixture = repo / "reference" / "persistence" / "fixtures" / "manifest-v2-storage-v1" / "METADATA.json"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("{}\n", encoding="utf-8")
        script = repo / "tools" / "check-persistence-fixture-immutability.sh"
        script.parent.mkdir(parents=True)
        script.write_bytes(PERSISTENCE_IMMUTABILITY.read_bytes())
        script.chmod(0o755)
        git(repo, "add", ".")
        git(repo, "commit", "-q", "-m", "fixture")
        fixture.write_text('{"changed": true}\n', encoding="utf-8")
        git(repo, "add", str(fixture.relative_to(repo)))

        env = os.environ.copy()
        env["PERSISTENCE_CHECK_INDEX"] = "1"
        result = subprocess.run(
            ["bash", str(script), "HEAD"],
            cwd=repo,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("historical persistence fixture is immutable", result.stdout)


if __name__ == "__main__":
    unittest.main()
