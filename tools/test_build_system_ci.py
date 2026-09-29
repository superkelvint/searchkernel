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


class BuildSystemCiTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="searchkernel-ci-gate.")
        self.root = pathlib.Path(self.temp.name)
        self.fake_cargo = self.root / "cargo"
        self.log = self.root / "cargo.log"
        self.fake_cargo.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            'if [[ "${1:-}" == "--version" ]]; then exit 0; fi\n'
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

    def test_ci_rejects_extra_arguments(self):
        result = self.run_ci("unexpected")
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage: ./dev ci", result.stderr)
        self.assertFalse(self.log.exists())

    def test_ci_uses_the_canonical_image_by_default(self):
        dev = DEV.read_text(encoding="utf-8")
        route = dev[dev.index("should_use_ci_image() {"):dev.index("# Build/test/verify commands")]
        self.assertIn("ci|check|universe", route)
        self.assertIn('exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"', dev)
        self.assertLess(
            dev.index('exec bash "$ROOT/tools/run-in-ci-image.sh" "$@"'),
            dev.index('PYTHON=$(bash "$ROOT/tools/resolve-python.sh")'),
        )

    def test_pr_workflow_delegates_without_owning_gate_semantics(self):
        workflow = RUST_CI.read_text(encoding="utf-8")
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn("actions/checkout@v4", workflow)
        self.assertIn("run: ./dev ci", workflow)
        checkout = workflow[
            workflow.index("- uses: actions/checkout@v4"):
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


if __name__ == "__main__":
    unittest.main()
