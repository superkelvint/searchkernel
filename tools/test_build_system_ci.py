#!/usr/bin/env python3
import json
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEV = ROOT / "dev"
RUST_CI = ROOT / ".github" / "workflows" / "rust-ci.yml"
TARGETS = ROOT / "build" / "targets.json"
CAPABILITIES_DOC = ROOT / "docs" / "CAPABILITIES.md"
SELECT_PYTHON = ROOT / ".github" / "ci" / "select-python.sh"
PERSISTENCE_IMMUTABILITY = ROOT / "tools" / "check-persistence-fixture-immutability.sh"


class BuildSystemCiTest(unittest.TestCase):
    def test_dev_ci_image_contract_runs_from_current_checkout(self):
        result = subprocess.run(
            ["bash", str(ROOT / "tools" / "test-dev-ci-image.sh")],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="searchkernel-ci-gate.")
        self.root = pathlib.Path(self.temp.name)
        self.fake_cargo = self.root / "cargo"
        self.log = self.root / "cargo.log"
        self.real_cargo = subprocess.check_output(
            ["bash", str(ROOT / "tools" / "resolve-rust.sh"), "cargo"],
            cwd=ROOT,
            text=True,
        ).strip()
        self.fake_cargo.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'if [[ "${1:-}" == "--version" ]]; then exit 0; fi\n'
            'if [[ "${1:-}" == "metadata" ]]; then exec "$CI_REAL_CARGO" "$@"; fi\n'
            'printf "%s\\n" "$*" >> "${CI_FAKE_CARGO_LOG}"\n'
            'if [[ "${CI_FAKE_CARGO_FAIL:-}" == "${1:-}" ]]; then\n'
            '  echo "FAKE_CARGO_FAILURE:${1}" >&2\n'
            "  exit 23\n"
            "fi\n",
            encoding="utf-8",
        )
        self.fake_cargo.chmod(0o755)

    def tearDown(self):
        self.temp.cleanup()

    def run_ci(self, *args, fail=""):
        environment = os.environ.copy()
        environment.update(
            {
                "CARGO": str(self.fake_cargo),
                "RUSTC": str(self.fake_cargo),
                "RUSTFMT": str(self.fake_cargo),
                "CI_REAL_CARGO": self.real_cargo,
                "CI_FAKE_CARGO_LOG": str(self.log),
                "CI_FAKE_CARGO_FAIL": fail,
                "SEARCHKERNEL_DEV_USE_CI_IMAGE": "0",
                "SEARCHKERNEL_IN_CI_IMAGE": "1",
            }
        )
        return subprocess.run(
            [str(DEV), "ci", *args],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
        )

    def test_ci_is_implemented_and_runs_the_exact_gate(self):
        registry = json.loads(TARGETS.read_text(encoding="utf-8"))
        self.assertEqual(registry["commands"]["ci"]["status"], "implemented")

        result = self.run_ci()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.log.read_text(encoding="utf-8").splitlines(),
            [
                "fmt --all -- --check",
                "check --workspace --all-targets --no-default-features --locked",
                "check -p searchkerneld --tests --features native --locked",
                "clippy --workspace --all-targets --no-default-features --locked -- -D warnings",
            ],
        )
        self.assertNotIn("not implemented", result.stderr)

    def test_ci_failure_output_and_status_propagate(self):
        result = self.run_ci(fail="check")
        self.assertEqual(result.returncode, 23)
        self.assertIn("FAKE_CARGO_FAILURE:check", result.stderr)
        self.assertEqual(
            self.log.read_text(encoding="utf-8").splitlines(),
            [
                "fmt --all -- --check",
                "check --workspace --all-targets --no-default-features --locked",
            ],
        )

    def test_ci_bookkeeping_checks_global_capability_ledger(self):
        dev = DEV.read_text(encoding="utf-8")
        start = dev.index("bookkeeping_command() {")
        end = dev.index("\ncheck_protocol() {", start)
        bookkeeping = dev[start:end]
        self.assertIn(
            '"$PYTHON" "$ROOT/tools/check-capabilities.py" --check',
            bookkeeping,
        )
        self.assertIn(
            '"$PYTHON" "$ROOT/tools/check-capabilities.py" --write',
            bookkeeping,
        )

    def test_ci_rejects_stale_global_capability_documents(self):
        original = CAPABILITIES_DOC.read_text(encoding="utf-8")
        try:
            CAPABILITIES_DOC.write_text(
                original + "\n<!-- deliberate stale capability document -->\n",
                encoding="utf-8",
            )
            result = self.run_ci()
        finally:
            CAPABILITIES_DOC.write_text(original, encoding="utf-8")

        self.assertEqual(result.returncode, 1)
        self.assertIn("Capability documents are stale:", result.stderr)
        self.assertFalse(self.log.exists())

    def test_ci_rejects_extra_arguments(self):
        result = self.run_ci("unexpected")
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage: ./dev ci", result.stderr)
        self.assertFalse(self.log.exists())

    def test_ci_uses_the_canonical_image_by_default(self):
        dev = DEV.read_text(encoding="utf-8")
        route = dev[dev.index("should_use_ci_image() {"):dev.index("# Build/test/verify commands")]
        self.assertIn(
            "ci|check|galaxy|universe|check-fast|check-protocol|coverage|lint|test|verify",
            route,
        )
        self.assertIn('exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"', dev)
        self.assertLess(
            dev.index('exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"'),
            dev.index('PYTHON=$(bash "$ROOT/tools/resolve-python.sh")'),
        )

    def test_pr_workflow_delegates_without_owning_gate_semantics(self):
        workflow = RUST_CI.read_text(encoding="utf-8")
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn("actions/checkout@v7", workflow)
        self.assertIn("run: ./dev ci", workflow)
        checkout = workflow[
            workflow.index("- uses: actions/checkout@v7"):
            workflow.index("- name: Run authoritative PR gate")
        ]
        run = workflow[workflow.index("- name: Run authoritative PR gate"):]
        self.assertIn("fetch-depth: 0", checkout)
        self.assertIn("path: source", checkout)
        self.assertIn("working-directory: source", run)
        for fragment in (
            "cargo fmt",
            "cargo check",
            "cargo clippy",
            "CARGO_TARGET_DIR",
            "FLATBUFFERS_CPP_ROOT",
            "FLATC_PATH",
            "docker run",
            "CI_IMAGE:",
            "CC=",
            "CXX=",
        ):
            self.assertNotIn(fragment, workflow)


    def test_abi_verifier_uses_repository_header_without_sdk_resolution(self):
        dev = DEV.read_text(encoding="utf-8")
        start = dev.index("        abi)\n")
        end = dev.index("        persistence)\n", start)
        abi = dev[start:end]

        self.assertIn(
            'git show HEAD:core/native/vespa/searchkernel_vespa.h > "$local_baseline"',
            abi,
        )
        self.assertIn(
            '"$ROOT/core/native/vespa/searchkernel_vespa.h"',
            abi,
        )
        self.assertNotIn("native-sdk ensure", abi)
        self.assertNotIn("resolve-native-sdk", abi)

    def test_ci_python_selection_never_uses_bare_system_python(self):
        selector = SELECT_PYTHON.read_text(encoding="utf-8")

        self.assertIn("sys.prefix != sys.base_prefix", selector)
        self.assertIn('if [[ -n "${CI_PYTHON:-}" ]] && isolated_python "$CI_PYTHON"; then', selector)
        self.assertIn('"$base" -m venv "$venv_dir"', selector)
        self.assertIn('if ! isolated_python "$selected"; then', selector)
        self.assertIn('echo "CI_PYTHON=$selected" >> "$GITHUB_ENV"', selector)
        self.assertNotIn('selected="$base"', selector)

    def test_persistence_immutability_can_check_the_staged_snapshot(self):
        repo = self.root / "persistence-repo"
        repo.mkdir()
        subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
        subprocess.run(
            ["git", "config", "user.name", "CI Test"],
            cwd=repo,
            check=True,
        )
        subprocess.run(
            ["git", "config", "user.email", "ci@example.invalid"],
            cwd=repo,
            check=True,
        )

        fixture = (
            repo
            / "reference"
            / "persistence"
            / "fixtures"
            / "manifest-v2-storage-v1"
            / "METADATA.json"
        )
        fixture.parent.mkdir(parents=True)
        fixture.write_text("{}\n", encoding="utf-8")

        script = repo / "tools" / "check-persistence-fixture-immutability.sh"
        script.parent.mkdir(parents=True)
        script.write_bytes(PERSISTENCE_IMMUTABILITY.read_bytes())
        script.chmod(0o755)

        subprocess.run(["git", "add", "."], cwd=repo, check=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "fixture"],
            cwd=repo,
            check=True,
        )

        fixture.write_text('{"changed": true}\n', encoding="utf-8")
        subprocess.run(
            ["git", "add", str(fixture.relative_to(repo))],
            cwd=repo,
            check=True,
        )

        environment = os.environ.copy()
        environment["PERSISTENCE_CHECK_INDEX"] = "1"
        result = subprocess.run(
            ["bash", str(script), "HEAD"],
            cwd=repo,
            env=environment,
            text=True,
            capture_output=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "historical persistence fixture is immutable",
            result.stdout + result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
