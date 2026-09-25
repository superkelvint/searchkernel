#!/usr/bin/env python3
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "ci-image-runner.yml"
WORKFLOWS = ROOT / ".github" / "workflows"


class NativeVerifierRunnerWorkflowTests(unittest.TestCase):
    def test_is_manual_or_reusable_but_not_direct_pr_execution(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("workflow_call:", source)
        self.assertIn("workflow_dispatch:", source)
        self.assertNotIn("pull_request:", source)
        self.assertNotIn("push:", source)
        self.assertNotIn("python3 verification/dag.py", source)
        self.assertNotIn("./verification/dag.py", source)

    def test_runs_requested_exact_revision_inside_ci_image(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "CI_IMAGE: docker.io/superkelvint/searchkernel-ci:latest",
            "persist-credentials: false",
            'docker pull "$CI_IMAGE"',
            'docker volume create "$SOURCE_VOLUME"',
            'docker volume create "$TARGET_VOLUME"',
            'git bundle create "$bundle" --all HEAD',
            'git clone --quiet "$bundle" /workspace',
            'git -C /workspace config core.hooksPath .githooks',
            'CI_IMAGE_ID=$image_id',
            'FLATBUFFERS_CPP_ROOT=/usr/local',
            'FLATC_PATH=/usr/local/bin/flatc',
            'CC=/opt/rh/gcc-toolset-15/root/usr/bin/gcc',
            'CXX=/opt/rh/gcc-toolset-15/root/usr/bin/g++',
            'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-15/root/usr/bin/gcc',
            'LIBRARY_PATH=/opt/rh/gcc-toolset-15/root/usr/lib64:/opt/rh/gcc-toolset-15/root/usr/lib',
            'test -f /opt/searchkernel/native-validation-pass',
            'native_base=$(./dev native-sdk path)',
            '/opt/searchkernel/native-sdk-cache/ci/*/objects/*',
            'command -v "$cmd"',
            "/usr/bin/python3 -c 'import clang.cindex'",
            './dev verify "$CI_TARGET"',
        ):
            self.assertIn(needle, source)

        for retired in (
            "./dev native-sdk ensure",
            "dnf install",
            "Resolve trusted native-base release",
            "/native-artifacts/",
            "SK_NATIVE_BASE_ARTIFACT_URL=",
            "NATIVE_CONTAINER: searchkernel-native",
            '-v "$GITHUB_WORKSPACE:/source:ro"',
            "cp -a /source/. /work/",
        ):
            self.assertNotIn(retired, source)

    def test_workers_share_no_mutable_toolchain_state(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for forbidden in (
            "searchkernel-ci-cargo-registry",
            "searchkernel-ci-cargo-git",
            "searchkernel-native-ci-lock",
            "/native-lock",
            ":/root/.cargo/registry",
            ":/root/.cargo/git",
            "ensure-runner-rust.sh",
            "Swatinem/rust-cache",
            "flock 9",
            '-v "$GITHUB_WORKSPACE:/source:ro"',
            "cp -a /source/. /work/",
            "/work/.git/searchkernel/toolchains",
        ):
            self.assertNotIn(forbidden, source)
        for required in (
            'SOURCE_VOLUME="searchkernel-ci-src-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${SAFE_TARGET}"',
            'TARGET_VOLUME="searchkernel-ci-target-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${SAFE_TARGET}"',
            'docker volume create "$SOURCE_VOLUME"',
            'docker volume create "$TARGET_VOLUME"',
            '-v "$SOURCE_VOLUME:/work"',
            '-v "$TARGET_VOLUME:/cargo-target"',
            "CARGO_TARGET_DIR=/cargo-target",
            "zero shared mutable worker state",
        ):
            self.assertIn(required, source)

    def test_supports_native_full_ci_bundle(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "native-full)",
            "./dev verify abi",
            "./dev verify vespa.nearest-neighbor.hnsw",
            "./dev verify product.integration",
            "./dev verify product.rust-real-engine",
            "./dev verify vespa.grouping.yql",
            "./dev verify vespa.recovery",
            "reference/vespa/phase9/verify-phase9.sh",
            "tools/run-python-native-e2e.sh",
            "./dev verify persistence",
        ):
            self.assertIn(needle, source)

    def test_supports_client_e2e_bundle_inside_ci_image(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "client-e2e)",
            '"$PYTHON" tools/check-python-dependencies.py',
            "build_native_searchkerneld client-e2e-build",
            'SEARCHKERNEL_LOCAL_HOST="$CARGO_TARGET_DIR/debug/searchkerneld"',
            'SEARCHKERNEL_SDK_LIB="$NATIVE_ADAPTER_LIB"',
            '"$PYTHON" tools/check-client-contracts.py --run-e2e',
        ):
            self.assertIn(needle, source)

    def test_native_searchkerneld_build_resolves_exact_adapter_runtime_lib(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "build_native_searchkerneld()",
            "--message-format=json-render-diagnostics",
            ".github/ci/resolve-native-adapter-lib.py",
            "NATIVE_ADAPTER_LIB=",
            "build_native_searchkerneld closure-422-build",
            'SEARCHKERNEL_SDK_LIB="$NATIVE_ADAPTER_LIB"',
        ):
            self.assertIn(needle, source)

    def test_supports_422_closure_bundle(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "closure-422)",
            "export CARGO_TARGET_DIR=/cargo-target/closure-422",
            "unset VESPA_LIBRARY_ROOT || true",
            "unset VESPA_DEPS || true",
            "unset VESPA_EXTRA_LIB_DIR || true",
            '/opt/searchkernel/native-sdk-cache/ci/*/objects/*',
            'closure-422 requires a checkout with no configured source Vespa tree',
            'bash tools/resolve-native-sdk.sh vespa-root',
            '/opt/searchkernel/native-sdk-cache/ci/*/toolchains/*/upstream/vespa',
            'build_native_searchkerneld closure-422-build',
            "./dev verify m14",
            "./dev verify product.rust-real-engine",
            "./dev verify integration",
            'bash clients/php/examples/run_searchkernel_client_api_e2e.sh',
        ):
            self.assertIn(needle, source)
        self.assertNotIn("SEARCHKERNEL_VERIFY_LIVE_ORACLE=skip", source)


    def _run_embedded_head_guard(
        self,
        payload: str,
        *,
        expected_head: str = "aaaaaaaa",
        pr_number: str = "123",
        curl_exit: int = 0,
    ) -> subprocess.CompletedProcess[str]:
        source = WORKFLOW.read_text(encoding="utf-8")
        start = source.index("          assert_current_pr_head() {")
        end = source.index("\n          assert_current_pr_head preflight", start)
        function_source = "\n".join(
            line[10:] if line.startswith("          ") else line
            for line in source[start:end].splitlines()
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            fake_curl = Path(temp_dir) / "curl"
            fake_curl.write_text(
                "#!/usr/bin/env bash\n"
                "printf '%s' \"$FAKE_CURL_PAYLOAD\"\n"
                "exit \"$FAKE_CURL_EXIT\"\n",
                encoding="utf-8",
            )
            fake_curl.chmod(0o755)
            env = os.environ.copy()
            env.update(
                {
                    "PATH": f"{temp_dir}:{env['PATH']}",
                    "FAKE_CURL_PAYLOAD": payload,
                    "FAKE_CURL_EXIT": str(curl_exit),
                }
            )
            script = f"""
set -euo pipefail
HEAD_CHECK_TOKEN=dummy
PR_NUMBER={pr_number!r}
EXPECTED_HEAD={expected_head!r}
GITHUB_REPOSITORY=superkelvint/searchkernel
GITHUB_API_URL=https://api.github.invalid
{function_source}
assert_current_pr_head test-phase
echo AFTER_GUARD
"""
            return subprocess.run(
                ["bash", "-c", script],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                env=env,
                check=False,
            )

    def test_head_guard_stops_closed_pr_even_when_sha_is_unchanged(self) -> None:
        completed = self._run_embedded_head_guard(
            '{"state":"closed","head":{"sha":"aaaaaaaa"}}'
        )
        self.assertEqual(completed.returncode, 0, completed.stdout)
        self.assertNotIn("AFTER_GUARD", completed.stdout)
        self.assertIn("stopping obsolete verification", completed.stdout)

    def test_head_guard_runtime_current_stale_and_api_failure(self) -> None:
        current = self._run_embedded_head_guard(
            '{"state":"open","head":{"sha":"aaaaaaaa"}}'
        )
        self.assertEqual(current.returncode, 0, current.stdout)
        self.assertIn("AFTER_GUARD", current.stdout)

        stale = self._run_embedded_head_guard(
            '{"state":"open","head":{"sha":"bbbbbbbb"}}'
        )
        self.assertEqual(stale.returncode, 0, stale.stdout)
        self.assertNotIn("AFTER_GUARD", stale.stdout)
        self.assertIn("Superseded verification", stale.stdout)

        malformed = self._run_embedded_head_guard(
            '{"head":{"sha":"aaaaaaaa"}}'
        )
        self.assertNotEqual(malformed.returncode, 0, malformed.stdout)
        self.assertNotIn("AFTER_GUARD", malformed.stdout)

        failed = self._run_embedded_head_guard("", curl_exit=22)
        self.assertNotEqual(failed.returncode, 0, failed.stdout)
        self.assertNotIn("AFTER_GUARD", failed.stdout)

        manual = self._run_embedded_head_guard(
            "",
            pr_number="",
            curl_exit=22,
        )
        self.assertEqual(manual.returncode, 0, manual.stdout)
        self.assertIn("AFTER_GUARD", manual.stdout)

    def test_superseded_pr_heads_stop_before_each_expensive_native_phase(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "pull-requests: read",
            'PR_NUMBER: ${{ github.event.pull_request.number }}',
            "assert_current_pr_head()",
            "curl --fail --silent --show-error",
            "--connect-timeout 5",
            "--max-time 15",
            'python3 -c \'import json,sys; data=json.load(sys.stdin); print(data.get("head", {}).get("sha", ""), data.get("state", ""))\'',
            'if [[ "$state" != "open" ]]; then',
            "env -u GH_TOKEN",
            "Superseded verification",
            "run_phase()",
            "assert_current_pr_head preflight",
            "run_phase native-full-abi",
            "run_phase native-full-hnsw",
            "run_phase native-full-product-integration",
            "run_phase native-full-rust-real-engine",
            "run_phase native-full-grouping-yql",
            "run_phase native-full-recovery",
            "run_phase native-full-phase9",
            "run_phase native-full-python-e2e",
            "run_phase native-full-persistence",
            'run_phase "$phase"',
            "build_native_searchkerneld client-e2e-build",
            "run_phase client-e2e-run",
            "build_native_searchkerneld closure-422-build",
            "run_phase closure-422-m14",
            "run_phase closure-422-rust-real-engine",
            "run_phase closure-422-integration",
            "run_phase closure-422-php-e2e",
            "run_phase requested-target",
        ):
            self.assertIn(needle, source)


    def test_pr_callers_grant_live_head_lookup_permission(self) -> None:
        callers: list[Path] = []
        workflows = sorted(
            [*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")]
        )
        for workflow in workflows:
            source = workflow.read_text(encoding="utf-8")
            if "uses: ./.github/workflows/ci-image-runner.yml" not in source:
                continue
            callers.append(workflow)
            header = source.split("\njobs:", 1)[0]
            self.assertIn("permissions:", header, workflow)
            self.assertIn("contents: read", header, workflow)
            self.assertIn("pull-requests: read", header, workflow)
        self.assertTrue(callers, "expected at least one ci-image-runner caller")


if __name__ == "__main__":
    unittest.main()
