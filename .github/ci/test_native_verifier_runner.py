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
        self.assertIn("closure-422)", source)
        self.assertIn("./dev verify m14", source)
        self.assertIn("./dev verify product.rust-real-engine", source)
        self.assertIn("./dev verify integration", source)


if __name__ == "__main__":
    unittest.main()
