#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("behavioral_lanes_required.py")
SPEC = importlib.util.spec_from_file_location("behavioral_lanes_required", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)

ROOT = Path(__file__).resolve().parents[2]
BEHAVIORAL_WORKFLOW = ROOT / ".github" / "workflows" / "behavioral-verification.yml"


class BehavioralLanesRequiredTests(unittest.TestCase):
    def assert_lanes(self, paths: list[str], expected: set[str]) -> None:
        selected, _reasons = POLICY.classify(paths)
        actual = {lane for lane, required in selected.items() if required}
        self.assertEqual(actual, expected)

    def test_client_local_changes_select_only_their_lane(self) -> None:
        cases = {
            "clients/python/searchkernel/client.py": {"python"},
            "clients/typescript/src/client.ts": {"typescript"},
            "clients/ruby/lib/searchkernel.rb": {"ruby"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assert_lanes([path], expected)

    def test_php_change_does_not_select_unrelated_behavioral_lanes(self) -> None:
        self.assert_lanes(["clients/php/src/Client.php"], set())

    def test_rust_local_change_selects_only_rust(self) -> None:
        self.assert_lanes(["core/crates/searchkernel-filter/src/lib.rs"], {"rust"})

    def test_multiple_client_changes_union_lanes(self) -> None:
        self.assert_lanes(
            [
                "clients/python/searchkernel/client.py",
                "clients/typescript/src/client.ts",
            ],
            {"python", "typescript"},
        )

    def test_portable_api_change_selects_all_lanes(self) -> None:
        self.assert_lanes(["api/searchkernel_request.fbs"], set(POLICY.LANES))

    def test_shared_client_contract_change_selects_all_lanes(self) -> None:
        self.assert_lanes(["tools/check-client-contracts.py"], set(POLICY.LANES))

    def test_ci_policy_change_selects_all_lanes(self) -> None:
        self.assert_lanes(
            [".github/ci/behavioral_lanes_required.py"], set(POLICY.LANES)
        )

    def test_documentation_only_change_selects_no_lanes(self) -> None:
        self.assert_lanes(["docs/architecture.md", "README.md"], set())

    def test_unknown_behavioral_path_fails_closed_to_all_lanes(self) -> None:
        self.assert_lanes(["integrations/new-runtime/bridge.rs"], set(POLICY.LANES))

    def test_behavioral_workflow_uses_trusted_base_selector(self) -> None:
        source = BEHAVIORAL_WORKFLOW.read_text(encoding="utf-8")
        for needle in (
            'rust: ${{ steps.behavioral.outputs.rust }}',
            'python: ${{ steps.behavioral.outputs.python }}',
            'typescript: ${{ steps.behavioral.outputs.typescript }}',
            'ruby: ${{ steps.behavioral.outputs.ruby }}',
            'git diff --name-only --no-renames "$BASE_SHA" "$HEAD_SHA"',
            'run: |',
            '"$CI_PYTHON" .github/ci/test_behavioral_lanes_required.py',
            'git cat-file -e "$BASE_SHA:.github/ci/behavioral_lanes_required.py"',
            'git show "$BASE_SHA:.github/ci/behavioral_lanes_required.py"',
            '"$CI_PYTHON" "$RUNNER_TEMP/behavioral_lanes_required.py" --github-output "$GITHUB_OUTPUT"',
            'echo "rust=true"',
            'echo "python=true"',
            'echo "typescript=true"',
            'echo "ruby=true"',
        ):
            self.assertIn(needle, source)
        self.assertNotIn("verification/dag.toml", source)

    def test_windows_separators_are_normalized(self) -> None:
        self.assert_lanes([r"clients\python\searchkernel\client.py"], {"python"})


if __name__ == "__main__":
    unittest.main()
