#!/usr/bin/env python3
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "on-demand-verification.yml"
TRUST_POLICY = ROOT / "docs" / "ci.md"


class OnDemandVerificationWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_uses_bounded_dispatch_and_pr_comment_ingress_with_isolated_reporter_write_permission(self):
        text = self.text
        self.assertIn("repository_dispatch:", text)
        self.assertIn("types: [searchkernel-verification]", text)
        self.assertIn("issue_comment:", text)
        self.assertIn("types: [created]", text)
        self.assertNotIn("workflow_dispatch:", text)
        self.assertNotRegex(text, r"(?m)^\s+pull_request:")
        self.assertNotRegex(text, r"(?m)^\s+push:")
        self.assertIn("permissions:\n  contents: read\n", text)
        verify_start = text.index("  verify:")
        reporter_start = text.index("  report-comment:")
        verify = text[verify_start:reporter_start]
        reporter = text[reporter_start:]
        self.assertNotIn("issues: write", verify)
        self.assertIn("permissions:\n      contents: read\n      issues: write\n", reporter)
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
        request = (ROOT / "tools" / "normalize-verification-request.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("^[0-9a-fA-F]{40}$", request)
        self.assertIn(
            "ref: ${{ steps.request.outputs.revision }}",
            text,
        )
        self.assertIn("persist-credentials: false", text)
        self.assertIn("observed=$(git rev-parse HEAD)", text)
        self.assertIn('if [[ "${observed}" != "${requested}" ]]', text)

    def test_subject_history_is_complete_for_canonical_git_bundle(self):
        text = self.text
        subject = text.index("Check out requested revision as untrusted subject")
        checkout = text[subject:text.index("Prove checked-out revision", subject)]
        runner = (ROOT / "tools" / "run-in-ci-image.sh").read_text(encoding="utf-8")
        self.assertIn("fetch-depth: 0", checkout)
        self.assertIn('git -C "$ROOT" bundle create "$bundle" HEAD', runner)

    def test_remote_surface_is_bounded_to_ci_test_and_verify(self):
        text = self.text
        request = (ROOT / "tools" / "normalize-verification-request.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn('case "$operation" in', request)
        self.assertIn("ci)", request)
        self.assertIn("test|verify)", request)
        self.assertIn('target" =~ ^[A-Za-z0-9._-]{1,128}$', request)
        self.assertIn('operation not in {"test", "verify"}', text)
        self.assertIn(
            'canonical}" =~ ^[A-Za-z0-9._-]{1,128}$',
            text,
        )
        self.assertNotIn("eval ", text)
        self.assertNotIn("eval ", request)

    def test_comment_ingress_is_prefiltered_and_uses_trusted_normalizer(self):
        text = self.text
        job = text[text.index("  verify:"):text.index("    runs-on:", text.index("  verify:"))]
        self.assertIn("github.event.issue.pull_request", job)
        self.assertIn("github.event.comment.author_association == 'OWNER'", job)
        self.assertIn("github.event.comment.author_association == 'MEMBER'", job)
        self.assertIn("github.event.comment.author_association == 'COLLABORATOR'", job)
        self.assertIn("startsWith(github.event.comment.body, '/searchkernel-verify ')", job)
        control = text.index("Check out trusted control plane")
        normalize = text.index("Normalize and validate authorized request")
        subject = text.index("Check out requested revision as untrusted subject")
        self.assertLess(control, normalize)
        self.assertLess(normalize, subject)
        self.assertIn(
            'bash "${GITHUB_WORKSPACE}/control/tools/normalize-verification-request.sh"',
            text,
        )

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
        self.assertIn('canonical_command = "./dev ci"', text)
        self.assertIn('canonical_command = f"./dev {operation} {target}"', text)
        self.assertIn('canonical_command = ""', text)

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

    def test_comment_triggered_results_are_posted_back_with_clickable_run_evidence(self):
        text = self.text
        reporter_start = text.index("  report-comment:")
        reporter = text[reporter_start:]

        self.assertIn("needs: verify", reporter)
        self.assertIn("always()", reporter)
        self.assertIn("github.event_name == 'issue_comment'", reporter)
        self.assertIn("github.event.issue.pull_request", reporter)
        self.assertIn("github.event.comment.author_association == 'OWNER'", reporter)
        self.assertIn("github.event.comment.author_association == 'MEMBER'", reporter)
        self.assertIn("github.event.comment.author_association == 'COLLABORATOR'", reporter)
        self.assertIn("github.rest.issues.createComment", reporter)
        self.assertIn(
            "RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}",
            reporter,
        )
        self.assertIn("REQUESTED_REVISION: ${{ needs.verify.outputs.requested_revision }}", reporter)
        self.assertIn("OBSERVED_REVISION: ${{ needs.verify.outputs.observed_revision }}", reporter)
        self.assertIn("CANONICAL_COMMAND: ${{ needs.verify.outputs.canonical_command }}", reporter)
        self.assertIn("VERIFICATION_RESULT: ${{ needs.verify.outputs.verification_result }}", reporter)
        self.assertIn("[View Actions run]", reporter)
        self.assertIn("PASS", reporter)
        self.assertIn("FAIL", reporter)
        self.assertNotIn("actions/checkout", reporter)
        self.assertNotIn("path: subject", reporter)

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
        self.assertIn("actions/upload-artifact@v7", text)
        self.assertIn("evidence.json", text)

    def test_evidence_does_not_claim_a_command_for_an_unresolved_target(self):
        text = self.text
        evidence_step = text.index("- name: Write structured evidence")
        script_start = text.index("python3 - <<'PY'", evidence_step) + len("python3 - <<'PY'\n")
        script_end = text.index("\n          PY", script_start)
        script = textwrap.dedent(text[script_start:script_end])
        with tempfile.TemporaryDirectory(prefix="searchkernel-evidence.") as raw_tmp:
            temp = pathlib.Path(raw_tmp)
            (temp / "searchkernel-verification").mkdir()
            environment = os.environ.copy()
            environment.update(
                {
                    "OPERATION": "test",
                    "CANONICAL_TARGET": "",
                    "REQUESTED_REVISION": "a" * 40,
                    "REQUEST_ID": "test-request",
                    "OBSERVED_REVISION": "a" * 40,
                    "RESULT": "not-run",
                    "RUN_ID": "123",
                    "RUN_ATTEMPT": "1",
                    "REPOSITORY": "superkelvint/searchkernel",
                    "SERVER_URL": "https://github.com",
                    "ACTOR": "test-actor",
                    "RUNNER_TEMP": str(temp),
                    "GITHUB_STEP_SUMMARY": str(temp / "summary.md"),
                }
            )
            subprocess.run(
                [sys.executable, "-c", script],
                check=True,
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
            )
            evidence_path = temp / "searchkernel-verification" / "evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            self.assertEqual(evidence["canonical_command"], "")

    def test_on_demand_runs_are_not_subject_to_concurrency_cancellation(self):
        text = self.text
        self.assertNotIn("cancel-in-progress:", text)
        self.assertNotRegex(text, r"(?m)^concurrency:")

    def test_minimal_trust_policy_is_documented(self):
        policy = " ".join(TRUST_POLICY.read_text(encoding="utf-8").split())
        for fragment in (
            "authenticated repository dispatch",
            "authorized pull-request comments",
            "OWNER, MEMBER, or COLLABORATOR",
            "exact commit SHAs available from this repository",
            "fork-only or",
            "read-only GitHub repository permissions",
            "no release, deployment, or write secrets",
            "no arbitrary",
        ):
            self.assertIn(fragment, policy)


if __name__ == "__main__":
    unittest.main()
