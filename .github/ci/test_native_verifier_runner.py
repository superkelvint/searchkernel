#!/usr/bin/env python3
from pathlib import Path
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


    def test_superseded_pr_heads_stop_before_each_expensive_native_phase(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "pull-requests: read",
            'PR_NUMBER: ${{ github.event.pull_request.number }}',
            "assert_current_pr_head()",
            "curl --fail --silent --show-error",
            "jq -r '.head.sha // empty'",
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
        for workflow in (BEHAVIORAL_WORKFLOW, CLIENT_CAPABILITIES_WORKFLOW):
            source = workflow.read_text(encoding="utf-8")
            self.assertIn("native-verifier-runner.yml@main", source)
            self.assertIn("permissions:", source)
            self.assertIn("contents: read", source)
            self.assertIn("pull-requests: read", source)


if __name__ == "__main__":
    unittest.main()
