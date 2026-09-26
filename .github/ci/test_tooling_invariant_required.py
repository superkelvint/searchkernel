#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("tooling_invariant_required.py")
SPEC = importlib.util.spec_from_file_location("tooling_invariant_required", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"
RUNNER = ROOT / ".github" / "workflows" / "ci-image-runner.yml"


class ToolingInvariantRequiredTests(unittest.TestCase):
    def test_tools_and_ci_policy_paths_require_tooling(self) -> None:
        for path in (
            "tools/test_ci_image_identity.py",
            "tools/test-dev-ci-image.sh",
            ".github/ci/behavioral_lanes_required.py",
            ".github/ci/test_exact_head.py",
            "reference/vespa/verifiers/test-routing.sh",
        ):
            with self.subTest(path=path):
                self.assertIsNotNone(POLICY.matching_rule(path))

    def test_product_and_documentation_paths_do_not_require_tooling(self) -> None:
        for path in (
            "core/crates/searchkernel-ir/src/lib.rs",
            "clients/python/searchkernel/client.py",
            "README.md",
            "docs/architecture.md",
            "dev",
        ):
            with self.subTest(path=path):
                self.assertIsNone(POLICY.matching_rule(path))

    def test_windows_separators_are_normalized(self) -> None:
        self.assertIsNotNone(POLICY.matching_rule(r"tools\\test_ci_image_identity.py"))

    def test_workflow_uses_trusted_base_selector_and_fails_closed(self) -> None:
        source = WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            'tooling: ${{ steps.tooling.outputs.required }}',
            'id: tooling',
            'selector="$RUNNER_TEMP/tooling_invariant_required.py"',
            'git show "$POLICY_SHA:.github/ci/tooling_invariant_required.py" > "$selector"',
            'cp .github/ci/tooling_invariant_required.py "$selector"',
            'echo "required=true" >> "$GITHUB_OUTPUT"',
        ):
            self.assertIn(needle.replace("\\$", "$"), source)
        self.assertNotIn('id: fixed', source)
        self.assertNotIn('echo "tooling=true" >> "$GITHUB_OUTPUT"', source)

    def test_tooling_lane_syntax_checks_all_tooling_sources(self) -> None:
        source = RUNNER.read_text(encoding="utf-8")
        tooling = source.split("            tooling)", 1)[1].split(
            "            client-capabilities)", 1
        )[0]
        self.assertIn(
            'run_phase tooling-python-syntax "$PYTHON" -m compileall -q tools .github/ci',
            tooling,
        )
        self.assertIn("run_phase ci-image-identity", tooling)
        self.assertIn("tools/test_ci_image_identity.py", tooling)


if __name__ == "__main__":
    unittest.main()
