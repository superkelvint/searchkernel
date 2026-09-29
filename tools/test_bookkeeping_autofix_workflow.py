#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "bookkeeping-autofix.yml"


class BookkeepingAutofixWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

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
        for path in (
            "build/targets.json",
            "docs/client-capabilities.json",
            "docs/CLIENT_CAPABILITIES.md",
        ):
            self.assertIn(path, self.text)
        self.assertIn("unauthorized path", self.text)
        self.assertIn("non-modification status", self.text)
        self.assertIn("git apply --check --whitespace=error-all", self.text)

    def test_autofix_uses_canonical_bookkeeping_commands(self):
        self.assertIn("./dev bookkeeping sync", self.text)
        self.assertIn("./dev bookkeeping check", self.text)

    def test_push_is_non_force_and_reuses_existing_autofix_credential(self):
        self.assertIn("secrets.RUSTFMT_AUTOFIX_TOKEN", self.text)
        self.assertNotIn("--force", self.text)
        self.assertIn("HEAD:refs/heads/${HEAD_REF}", self.text)


if __name__ == "__main__":
    unittest.main()
