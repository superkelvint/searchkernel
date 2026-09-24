#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("native_verifier_required.py")
SPEC = importlib.util.spec_from_file_location("native_verifier_required", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


ROOT = Path(__file__).resolve().parents[2]
BEHAVIORAL_WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"


class NativeVerifierRequiredTests(unittest.TestCase):
    def test_native_sensitive_paths_are_selected(self) -> None:
        paths = [
            "core/native/searchkernel_vespa.cpp",
            "core/crates/searchkernel-vespa-engine/src/lib.rs",
            "core/crates/searchkernel-vespa-bindings/src/lib.rs",
            "core/crates/searchkernel-ir-adapter/src/search.rs",
            "core/crates/searchkernel/src/index/native.rs",
            "core/crates/searchkernel/src/search.rs",
            "core/crates/searchkernel-ir/src/snapshot.rs",
            "searchkerneld/src/main.rs",
            "api/searchkernel_response.fbs",
            "reference/persistence/v3/manifest.json",
            "reference/vespa/verifiers/results/verify.sh",
            "tools/check_abi_surface.py",
            "tools/fetch-native-base.sh",
            "tools/actions-runner-setup.sh",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".github/workflows/native-verifier-runner.yml",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assertIsNotNone(POLICY.matching_rule(path))

    def test_non_native_paths_do_not_consume_native_runner(self) -> None:
        paths = [
            "clients/python/searchkernel/client.py",
            "clients/typescript/src/client.ts",
            "clients/ruby/lib/searchkernel.rb",
            "dev",
            "searchkernel-web/src/features/overview/index.tsx",
            "core/crates/searchkernel-filter/src/lib.rs",
            "core/crates/searchkernel-dispatch/src/lib.rs",
            "docs/architecture.md",
            "README.md",
            "verification/dag.toml",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assertIsNone(POLICY.matching_rule(path))

    def test_ruby_client_plus_dev_does_not_consume_native_runner(self) -> None:
        matches = POLICY.classify(
            [
                "clients/ruby/lib/searchkernel/value.rb",
                "core/crates/searchkernel-protocol/tests/ruby_json_contract.rs",
                "dev",
            ]
        )
        self.assertEqual(matches, [])

    def test_rename_safe_diff_can_classify_deleted_native_path(self) -> None:
        # CI uses git diff --no-renames, so a move out of a native-sensitive
        # path still exposes the deleted source path to this classifier.
        matches = POLICY.classify(
            ["core/native/old_adapter.cpp", "docs/old_adapter-notes.md"]
        )
        self.assertEqual(matches[0][0], "core/native/old_adapter.cpp")

    def test_behavioral_workflow_uses_trusted_base_selector_and_native_gate(self) -> None:
        source = BEHAVIORAL_WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            'git diff --name-only --no-renames "$BASE_SHA" "$HEAD_SHA"',
            'git show "$BASE_SHA:.github/ci/native_verifier_required.py"',
            'uses: superkelvint/searchkernel/.github/workflows/native-verifier-runner.yml@main',
            'target: native-full',
            'needs: [classify, rust, python, typescript, ruby, native]',
            'require_lane Native "$NATIVE_SENSITIVE" "$NATIVE_RESULT"',
        ):
            self.assertIn(needle, source)
        self.assertNotIn("verification/dag.toml", source)


if __name__ == "__main__":
    unittest.main()
