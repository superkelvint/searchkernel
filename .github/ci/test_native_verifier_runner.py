#!/usr/bin/env python3
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "native-verifier-runner.yml"


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
            'closure-422 requires a checkout with no configured source Vespa tree',
            'bash tools/resolve-native-sdk.sh vespa-root',
            'cargo build -p searchkerneld --features native',
            "./dev verify m14",
            "./dev verify product.rust-real-engine",
            "./dev verify integration",
            'bash clients/php/examples/run_searchkernel_client_api_e2e.sh',
        ):
            self.assertIn(needle, source)

        # Closure evidence must prove the automatic published-provenance path,
        # not force the verifier into frozen-oracle mode.
        self.assertNotIn(
            "SEARCHKERNEL_VERIFY_LIVE_ORACLE=skip",
            source,
        )


if __name__ == "__main__":
    unittest.main()
