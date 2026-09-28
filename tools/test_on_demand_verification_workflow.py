#!/usr/bin/env python3
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "on-demand-verification.yml"


class OnDemandVerificationWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_is_manual_only_and_read_only(self):
        text = self.text
        self.assertIn("workflow_dispatch:", text)
        self.assertNotRegex(text, r"(?m)^\s+pull_request:")
        self.assertNotRegex(text, r"(?m)^\s+push:")
        self.assertRegex(text, r"(?ms)^permissions:\n\s+contents: read\s*$")

    def test_control_plane_must_run_from_default_branch(self):
        text = self.text
        guard = text.index("Require trusted default-branch control plane")
        checkout = text.index("Check out requested revision as untrusted subject")
        self.assertLess(guard, checkout)
        self.assertIn('expected="refs/heads/${DEFAULT_BRANCH}"', text)
        self.assertIn('if [[ "${GITHUB_REF}" != "${expected}" ]]', text)

    def test_revision_is_exact_sha_and_checkout_is_proven(self):
        text = self.text
        self.assertIn("^[0-9a-fA-F]{40}$", text)
        self.assertIn("ref: ${{ inputs.revision }}", text)
        self.assertIn("persist-credentials: false", text)
        self.assertIn("observed=\$(git rev-parse HEAD)", text)
        self.assertIn('if [[ "${observed}" != "${requested}" ]]', text)

    def test_remote_surface_is_bounded_to_test_and_verify_for_now(self):
        text = self.text
        operation_block = text[
            text.index("operation:") : text.index("target:", text.index("operation:"))
        ]
        self.assertIn("- test", operation_block)
        self.assertIn("- verify", operation_block)
        self.assertNotIn("- ci", operation_block)
        self.assertIn('operation not in {"test", "verify"}', text)
        self.assertNotIn("eval ", text)

    def test_target_validation_uses_shared_registry_authority(self):
        text = self.text
        validate = text.index("Validate target against repository authority")
        execute = text.index("Run canonical verification")
        self.assertLess(validate, execute)
        self.assertIn('pathlib.Path("build/targets.json")', text)
        self.assertIn('metadata.get("aliases", [])', text)
        self.assertIn('./dev "${OPERATION}" "${CANONICAL_TARGET}"', text)

    def test_structured_evidence_is_always_published(self):
        text = self.text
        for field in (
            '"requested_revision"',
            '"revision"',
            '"operation"',
            '"target"',
            '"canonical_command"',
            '"result"',
            '"run_id"',
            '"run_attempt"',
            '"run_url"',
        ):
            self.assertIn(field, text)
        self.assertGreaterEqual(text.count("if: always()"), 2)
        self.assertIn("actions/upload-artifact@v4", text)
        self.assertIn("evidence.json", text)

    def test_concurrency_does_not_cancel_exact_sha_evidence(self):
        text = self.text
        self.assertIn("on-demand-verification-${{ inputs.revision }}", text)
        self.assertIn("cancel-in-progress: false", text)


if __name__ == "__main__":
    unittest.main()
