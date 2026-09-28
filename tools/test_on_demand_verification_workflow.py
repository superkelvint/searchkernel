#!/usr/bin/env python3
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "on-demand-verification.yml"
TRUST_POLICY = ROOT / "docs" / "ci.md"


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
        self.assertNotIn("${{ secrets.", text)

    def test_control_plane_is_proven_before_subject_checkout(self):
        text = self.text
        guard = text.index("Prove trusted default-branch control plane")
        checkout = text.index("Check out requested revision as untrusted subject")
        self.assertLess(guard, checkout)
        self.assertIn('expected="refs/heads/${DEFAULT_BRANCH}"', text)
        self.assertIn('if [[ "${GITHUB_REF}" != "${expected}" ]]', text)
        self.assertIn("ref: ${{ github.sha }}", text)
        self.assertIn("path: control", text)

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

    def test_remote_surface_is_bounded_to_ci_test_and_verify(self):
        text = self.text
        self.assertIn('case "${OPERATION}" in', text)
        self.assertIn("ci)", text)
        self.assertIn("test|verify)", text)
        self.assertIn('operation not in {"test", "verify"}', text)
        self.assertIn('if [[ -n "${REQUESTED_TARGET}" ]]', text)
        self.assertIn(
            'REQUESTED_TARGET}" =~ ^[A-Za-z0-9._-]{1,128}$',
            text,
        )
        self.assertIn(
            'canonical}" =~ ^[A-Za-z0-9._-]{1,128}$',
            text,
        )
        self.assertNotIn("eval ", text)

    def test_ci_has_no_target_and_test_verify_keep_git_object_validation(self):
        text = self.text
        target = text.index("Validate target against repository authority")
        validation = text[target:text.index("Run canonical verification", target)]
        self.assertIn('if [[ "${OPERATION}" == "ci" ]]; then', validation)
        self.assertIn("printf 'target=\\n'", validation)
        self.assertLess(
            validation.index('if [[ "${OPERATION}" == "ci" ]]'),
            validation.index("git ls-tree HEAD -- build/targets.json"),
        )
        self.assertIn("git show HEAD:build/targets.json", validation)
        self.assertIn('operation not in {"test", "verify"}', validation)

        execute = text[text.index("Run canonical verification"):]
        self.assertIn("canonical command: ./dev ci", execute)
        self.assertIn('run-in-ci-image.sh" ci', execute)
        self.assertIn('run-in-ci-image.sh" "${OPERATION}" "${CANONICAL_TARGET}"', execute)
        self.assertIn('"canonical_command": f"./dev {operation} {target}".rstrip()', text)

    def test_requested_revision_must_exist_in_this_repository(self):
        text = self.text
        control = text.index("Check out trusted control plane")
        provenance = text.index("Verify requested commit belongs to this repository")
        subject = text.index("Check out requested revision as untrusted subject")
        self.assertLess(control, provenance)
        self.assertLess(provenance, subject)
        self.assertIn("github.rest.repos.getCommit", text)
        self.assertIn("requested revision ${revision} is unavailable", text)

    def test_target_validation_uses_git_object_without_dereferencing_subject_path(self):
        text = self.text
        validate = text.index("Validate target against repository authority")
        execute = text.index("Run canonical verification")
        self.assertLess(validate, execute)
        self.assertIn("git ls-tree HEAD -- build/targets.json", text)
        self.assertIn('registry_mode}" != "100644"', text)
        self.assertIn('registry_type}" != "blob"', text)
        self.assertIn('git cat-file -s "${registry_object}"', text)
        self.assertIn("registry_size > 1048576", text)
        self.assertIn("git show HEAD:build/targets.json", text)
        self.assertIn("json.load(sys.stdin, object_pairs_hook=reject_duplicates)", text)
        self.assertNotIn('pathlib.Path("build/targets.json")', text)
        self.assertNotIn("registry_path.read_text", text)
        self.assertIn('metadata.get("aliases", [])', text)
        self.assertIn(
            "SEARCHKERNEL_CI_SOURCE_ROOT: ${{ github.workspace }}/subject",
            text,
        )
        self.assertIn('SEARCHKERNEL_CI_EPHEMERAL_TARGET: "1"', text)
        self.assertIn(
            'bash "${GITHUB_WORKSPACE}/control/tools/run-in-ci-image.sh" "${OPERATION}" "${CANONICAL_TARGET}"',
            text,
        )
        self.assertNotIn(
            '\n          ./dev "${OPERATION}" "${CANONICAL_TARGET}"',
            text,
        )

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

    def test_minimal_trust_policy_is_documented(self):
        policy = " ".join(TRUST_POLICY.read_text(encoding="utf-8").split())
        for fragment in (
            "authenticated repository dispatch",
            "exact commit SHAs available from this repository",
            "fork-only or",
            "read-only GitHub repository permissions",
            "no release, deployment, or write secrets",
            "no arbitrary",
            "separate actor allowlist",
        ):
            self.assertIn(fragment, policy)


if __name__ == "__main__":
    unittest.main()
