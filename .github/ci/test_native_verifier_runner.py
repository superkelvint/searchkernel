#!/usr/bin/env python3
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "native-verifier-runner.yml"
BEHAVIORAL_WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"
CLIENT_CAPABILITIES_WORKFLOW = ROOT / ".github" / "workflows" / "client-capabilities.yml"


class NativeVerifierRunnerWorkflowTests(unittest.TestCase):
    def test_is_manual_or_reusable_but_not_direct_pr_execution(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("workflow_call:", source)
        self.assertIn("workflow_dispatch:", source)
        self.assertNotIn("pull_request:", source)
        self.assertNotIn("push:", source)
        self.assertNotIn("verification/dag", source)

    def test_runs_requested_exact_revision_inside_persistent_alma(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "NATIVE_CONTAINER: searchkernel-native",
            "persist-credentials: false",
            'docker cp "$GITHUB_WORKSPACE/." "$NATIVE_CONTAINER:$ALMA_WORKTREE"',
            'git -c safe.directory="$ALMA_WORKTREE" -C "$ALMA_WORKTREE" rev-parse HEAD',
            './dev native-sdk ensure',
            'export CARGO_HOME="$ALMA_WORKTREE/.cargo-home"',
            'ln -s "/root/.cargo/$cache" "$CARGO_HOME/$cache"',
            "dnf install -y clang python3-clang",
            "command -v clang",
            "python3 -c 'import clang.cindex'",
            './dev verify "$VERIFIER_TARGET"',
        ):
            self.assertIn(needle, source)

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

    def test_supports_client_e2e_bundle_inside_alma(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "client-e2e)",
            "python3 tools/check-python-dependencies.py",
            "cargo build -p searchkerneld --features native",
            'SEARCHKERNEL_LOCAL_HOST="$CARGO_TARGET_DIR/debug/searchkerneld"',
            "python3 tools/check-client-contracts.py --run-e2e",
        ):
            self.assertIn(needle, source)

    def test_supports_422_closure_bundle(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "closure-422)",
            'export SK_NATIVE_CACHE_DIR="$ALMA_WORKTREE/.native-cache"',
            'export CARGO_TARGET_DIR="$ALMA_WORKTREE/target/closure-422"',
            "unset VESPA_LIBRARY_ROOT || true",
            "unset VESPA_DEPS || true",
            "unset VESPA_EXTRA_LIB_DIR || true",
            'rm -rf "$SK_NATIVE_CACHE_DIR" "$CARGO_TARGET_DIR"',
            'artifact_mount_rw=$(docker inspect',
            "Resolve trusted native-base release",
            'gh api --paginate "repos/${GITHUB_REPOSITORY}/releases?per_page=100"',
            'target=$(docker exec "$NATIVE_CONTAINER"',
            'EXPECTED_NATIVE_ARCHIVE_NAME: ${{ steps.native_base.outputs.archive }}',
            'artifact="/native-artifacts/$EXPECTED_NATIVE_ARCHIVE_NAME"',
            'native base digest does not match trusted release identity',
            "ldd sha256sum",
            'closure-422 requires a checkout with no configured source Vespa tree',
            'bash tools/resolve-native-sdk.sh vespa-root',
            'cargo build -p searchkerneld --features native',
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
        self.assertIn("stopping obsolete native verification", completed.stdout)

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
        self.assertIn("Superseded native verification", stale.stdout)

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
            'python3 -c \'import json,sys; data=json.load(sys.stdin); print(data.get("head", {}).get("sha", ""), data.get("state", ""))\'',
            'if [[ "$state" != "open" ]]; then',
            "env -u GH_TOKEN",
            "Superseded native verification",
            "run_native_phase()",
            "assert_current_pr_head preflight",
            "run_native_phase native-sdk",
            "run_native_phase native-full-abi",
            "run_native_phase native-full-hnsw",
            "run_native_phase native-full-product-integration",
            "run_native_phase native-full-rust-real-engine",
            "run_native_phase native-full-grouping-yql",
            "run_native_phase native-full-recovery",
            "run_native_phase native-full-phase9",
            "run_native_phase native-full-python-e2e",
            "run_native_phase native-full-persistence",
            "run_native_phase client-e2e-build",
            "run_native_phase client-e2e-run",
            "run_native_phase closure-422-build",
            "run_native_phase closure-422-m14",
            "run_native_phase closure-422-rust-real-engine",
            "run_native_phase closure-422-integration",
            "run_native_phase closure-422-php-e2e",
            "run_native_phase requested-target",
            "flock 9",
        ):
            self.assertIn(needle, source)


    def test_pr_callers_grant_live_head_lookup_permission(self) -> None:
        callers: list[Path] = []
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            source = workflow.read_text(encoding="utf-8")
            if "native-verifier-runner.yml@main" not in source:
                continue
            callers.append(workflow)
            header = source.split("\njobs:", 1)[0]
            self.assertIn("permissions:", header, workflow)
            self.assertIn("contents: read", header, workflow)
            self.assertIn("pull-requests: read", header, workflow)
        self.assertTrue(callers, "expected at least one native-verifier caller")


if __name__ == "__main__":
    unittest.main()
