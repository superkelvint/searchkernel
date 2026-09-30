#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "bookkeeping-autofix.yml"
RUSTFMT_WORKFLOW = ROOT / ".github" / "workflows" / "rustfmt-autofix.yml"


class BookkeepingAutofixWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")
        self.rustfmt_text = RUSTFMT_WORKFLOW.read_text(encoding="utf-8")

    def test_uses_pull_request_target_but_only_for_same_repo_heads(self):
        self.assertIn("pull_request_target:", self.text)
        self.assertGreaterEqual(
            self.text.count(
                "github.event.pull_request.head.repo.full_name == github.repository"
            ),
            2,
        )

    def test_untrusted_checkout_has_no_credentials_and_is_exact_sha(self):
        self.assertGreaterEqual(
            self.text.count("ref: ${{ github.event.pull_request.head.sha }}"), 2
        )
        self.assertGreaterEqual(self.text.count("persist-credentials: false"), 2)
        self.assertIn("EXPECTED_HEAD: ${{ github.event.pull_request.head.sha }}", self.text)
        self.assertIn('if [[ "${actual,,}" != "${EXPECTED_HEAD,,}" ]]', self.text)

    def test_patch_scope_is_fail_closed(self):
        expected_paths = {
            "build/targets.json",
            "LIMITATIONS.md",
            "docs/CAPABILITIES.md",
            "docs/client-capabilities.json",
            "docs/CLIENT_CAPABILITIES.md",
        }
        match = re.search(
            r'case "\$path" in\s*\n\s*(?P<paths>[^\n)]+)\)\s*\n\s*;;',
            self.text,
        )
        self.assertIsNotNone(match)
        assert match is not None
        self.assertEqual(
            set(match.group("paths").strip().split("|")),
            expected_paths,
            "bookkeeping autofix allowlist must contain only deterministic outputs",
        )
        self.assertIn("unauthorized path", self.text)
        self.assertIn("non-modification status", self.text)
        self.assertIn("git apply --check --whitespace=error-all", self.text)

    def test_autofix_uses_canonical_bookkeeping_commands(self):
        self.assertIn("./dev bookkeeping sync", self.text)
        self.assertIn("./dev bookkeeping check", self.text)

    def test_untrusted_execution_is_containerized_without_network(self):
        self.assertGreaterEqual(self.text.count("docker run --rm --network none"), 2)
        self.assertIn('docker.io/superkelvint/searchkernel-ci:latest', self.text)
        self.assertIn('-v "$GITHUB_WORKSPACE/source:/work"', self.text)
        self.assertIn('-v "$GITHUB_WORKSPACE/source:/work:ro"', self.text)
        self.assertIn("SEARCHKERNEL_IN_CI_IMAGE=1", self.text)
        self.assertIn("PYTHONDONTWRITEBYTECODE=1", self.text)

    def test_uses_current_action_versions_and_rerun_safe_artifacts(self):
        self.assertGreaterEqual(self.text.count("actions/checkout@v7"), 2)
        self.assertIn("actions/upload-artifact@v7", self.text)
        self.assertIn("actions/download-artifact@v8", self.text)
        self.assertIn("name: bookkeeping-patch-${{ github.run_id }}", self.text)
        self.assertIn("overwrite: true", self.text)
        self.assertNotIn("github.run_attempt", self.text)
        self.assertGreaterEqual(self.text.count("path: source"), 2)

    def test_push_is_non_force_and_reuses_existing_autofix_credential(self):
        for text in (self.text, self.rustfmt_text):
            self.assertIn("secrets.RUSTFMT_AUTOFIX_TOKEN", text)
            self.assertNotIn("--force", text)
            self.assertIn("HEAD:refs/heads/${HEAD_REF}", text)

    def test_parallel_autofix_pushes_converge_when_head_advances(self):
        for text in (self.text, self.rustfmt_text):
            self.assertIn('git -c credential.helper= ls-remote', text)
            self.assertIn('"${remote_head,,}" != "${EXPECTED_HEAD,,}"', text)
            self.assertIn("autofix superseded by newer PR head", text)
            self.assertIn('exit "$push_status"', text)


if __name__ == "__main__":
    unittest.main()
