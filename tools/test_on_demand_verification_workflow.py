#!/usr/bin/env python3
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "on-demand-verification.yml"


class OnDemandVerificationWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_uses_default_branch_repository_dispatch_and_read_only_permissions(self):
        text = self.text
        self.assertIn("repository_dispatch:", text)
        self.assertIn("types: [searchkernel-verification]", text)
        self.assertNotIn("workflow_dispatch:", text)
        self.assertNotRegex(text, r"(?m)^\s+pull_request:")
        self.assertNotRegex(text, r"(?m)^\s+push:")
        self.assertIn("permissions:\n  contents: read\n", text)

    def test_control_plane_is_proven_before_subject_checkout(self):
        text = self.text
        guard = text.index("Prove trusted default-branch control plane")
        checkout = text.index("Check out requested revision as untrusted subject")
        self.assertLess(guard, checkout)
        self.assertIn('expected="refs/heads/${DEFAULT_BRANCH}"', text)
        self.assertIn('if [[ "${GITHUB_REF}" != "${expected}" ]]', text)

    def test_revision_is_exact_sha_and_checkout_is_proven(self):
        text = self.text
        self.assertIn("^[0-9a-fA-F]{40}$", text)
        self.assertIn(
            "ref: ${{ github.event.client_payload.revision }}",
            text,
        )
        self.assertIn("persist-credentials: false", text)
        self.assertIn("observed=$(git rev-parse HEAD)", text)
        self.assertIn('if [[ "${observed}" != "${requested}" ]]', text)

    def test_remote_surface_is_bounded_to_test_and_verify_for_now(self):
        text = self.text
        self.assertIn('case "${OPERATION}" in', text)
        self.assertIn("test|verify)", text)
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
            '"request_id"',
            '"revision"',
            '"operation"',
            '"target"',
            '"canonical_command"',
            '"result"',
            '"actor"',
            '"run_id"',
            '"run_attempt"',
            '"run_url"',
        ):
            self.assertIn(field, text)
        self.assertGreaterEqual(text.count("if: always()"), 2)
        self.assertIn("actions/upload-artifact@v4", text)
        self.assertIn("evidence.json", text)

    def test_on_demand_runs_are_not_subject_to_concurrency_cancellation(self):
        text = self.text
        self.assertNotIn("cancel-in-progress:", text)
        self.assertNotRegex(text, r"(?m)^concurrency:")


if __name__ == "__main__":
    unittest.main()
