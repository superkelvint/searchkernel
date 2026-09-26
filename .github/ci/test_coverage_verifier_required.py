#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("coverage_verifier_required.py")
SPEC = importlib.util.spec_from_file_location("coverage_verifier_required", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


ROOT = Path(__file__).resolve().parents[2]
BEHAVIORAL_WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"


class CoverageVerifierRequiredTests(unittest.TestCase):
    def test_coverage_execution_surface_is_selected(self) -> None:
        paths = [
            "dev",
            "docker/searchkernel-ci/Dockerfile",
            "tools/coverage.sh",
            "tools/run-in-ci-image.sh",
            "tools/test-dev-ci-image.sh",
            ".github/workflows/ci-image-runner.yml",
            ".github/workflows/nightly.yml",
            ".github/ci/coverage_verifier_required.py",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assertIsNotNone(POLICY.matching_rule(path))

    def test_unrelated_product_changes_do_not_run_full_coverage(self) -> None:
        paths = [
            "core/crates/searchkernel-ir/src/lib.rs",
            "clients/python/searchkernel/client.py",
            "clients/typescript/src/client.ts",
            "clients/ruby/lib/searchkernel.rb",
            "docs/architecture.md",
            "README.md",
        ]
        for path in paths:
            with self.subTest(path=path):
                self.assertIsNone(POLICY.matching_rule(path))

    def test_behavioral_workflow_requires_real_exact_head_coverage_lane(self) -> None:
        source = BEHAVIORAL_WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            "coverage: ${{ steps.coverage.outputs.required }}",
            'git show "$POLICY_SHA:.github/ci/coverage_verifier_required.py" > "$selector"',
            "target: nightly-coverage",
            "needs: [classify, rust, python, typescript, ruby, tooling, native, coverage]",
            'require_lane Coverage "$COVERAGE_REQUIRED" "$COVERAGE_RESULT"',
        ):
            self.assertIn(needle, source)


if __name__ == "__main__":
    unittest.main()
