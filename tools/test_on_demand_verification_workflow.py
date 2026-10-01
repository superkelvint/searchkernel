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
REQUEST_NORMALIZER = ROOT / "tools" / "normalize-verification-request.sh"


class OnDemandVerificationWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_uses_bounded_dispatch_and_pr_comment_ingress_with_read_only_permissions(self):
        text = self.text
        self.assertIn("repository_dispatch:", text)
        self.assertIn("types: [searchkernel-verification]", text)
        self.assertIn("issue_comment:", text)
        self.assertIn("types: [created]", text)
        self.assertNotIn("workflow_dispatch:", text)
        self.assertNotRegex(text, r"(?m)^\s+pull_request:")
        self.assertNotRegex(text, r"(?m)^\s+push:")
        self.assertIn("permissions:\n  contents: read\n", text)
        self.assertNotIn("${{ secrets.", text)

    def test_comment_ingress_is_prefiltered_and_uses_trusted_default_branch_normalizer(self):
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
        request = REQUEST_NORMALIZER.read_text(encoding="utf-8")
        self.assertIn('case "$operation" in', request)
        self.assertIn("ci)", request)
        self.assertIn("test|verify)", request)
        self.assertIn('target" =~ ^[A-Za-z0-9._-]{1,128}
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



class VerificationRequestNormalizerTest(unittest.TestCase):
    def run_request(self, **values):
        environment = os.environ.copy()
        environment.update(
            {
                "EVENT_NAME": "",
                "DISPATCH_REVISION": "",
                "DISPATCH_OPERATION": "",
                "DISPATCH_TARGET": "",
                "DISPATCH_REQUEST_ID": "",
                "COMMENT_BODY": "",
                "COMMENT_ID": "",
                "COMMENT_ASSOCIATION": "",
                "PULL_REQUEST_URL": "",
            }
        )
        environment.update(values)
        with tempfile.TemporaryDirectory(prefix="searchkernel-request.") as raw_tmp:
            output = pathlib.Path(raw_tmp) / "github-output"
            environment["GITHUB_OUTPUT"] = str(output)
            result = subprocess.run(
                ["bash", str(REQUEST_NORMALIZER)],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
            )
            outputs = {}
            if output.exists():
                for line in output.read_text(encoding="utf-8").splitlines():
                    key, value = line.split("=", 1)
                    outputs[key] = value
            return result, outputs

    def test_repository_dispatch_normalizes_without_semantic_change(self):
        revision = "a" * 40
        result, outputs = self.run_request(
            EVENT_NAME="repository_dispatch",
            DISPATCH_REVISION=revision,
            DISPATCH_OPERATION="verify",
            DISPATCH_TARGET="vespa.ranking",
            DISPATCH_REQUEST_ID="remote-test-request",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            outputs,
            {
                "revision": revision,
                "operation": "verify",
                "target": "vespa.ranking",
                "request_id": "remote-test-request",
            },
        )

    def test_authorized_pr_comment_requests_test_and_ci(self):
        revision = "b" * 40
        common = {
            "EVENT_NAME": "issue_comment",
            "COMMENT_ASSOCIATION": "MEMBER",
            "PULL_REQUEST_URL": "https://api.github.com/repos/superkelvint/searchkernel/pulls/1",
        }
        result, outputs = self.run_request(
            **common,
            COMMENT_BODY=f"/searchkernel-verify test ruby {revision}",
            COMMENT_ID="12345",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            outputs,
            {
                "revision": revision,
                "operation": "test",
                "target": "ruby",
                "request_id": "comment-12345",
            },
        )

        result, outputs = self.run_request(
            **common,
            COMMENT_BODY=f"/searchkernel-verify ci {revision}",
            COMMENT_ID="12346",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["operation"], "ci")
        self.assertEqual(outputs["target"], "")

    def test_comment_request_rejects_unauthorized_non_pr_and_multiline_callers(self):
        revision = "c" * 40
        base = {
            "EVENT_NAME": "issue_comment",
            "COMMENT_ID": "99",
            "COMMENT_BODY": f"/searchkernel-verify test ruby {revision}",
            "PULL_REQUEST_URL": "https://api.github.com/repos/superkelvint/searchkernel/pulls/1",
        }
        cases = [
            ({**base, "COMMENT_ASSOCIATION": "CONTRIBUTOR"}, "not authorized"),
            ({**base, "COMMENT_ASSOCIATION": "MEMBER", "PULL_REQUEST_URL": ""}, "must be made on a pull request"),
            (
                {**base, "COMMENT_ASSOCIATION": "MEMBER", "COMMENT_BODY": base["COMMENT_BODY"] + "\nignored"},
                "exactly one line",
            ),
        ]
        for environment, message in cases:
            with self.subTest(message=message):
                result, outputs = self.run_request(**environment)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertEqual(outputs, {})

    def test_comment_request_rejects_bad_shape_sha_and_target(self):
        revision = "d" * 40
        base = {
            "EVENT_NAME": "issue_comment",
            "COMMENT_ID": "100",
            "COMMENT_ASSOCIATION": "OWNER",
            "PULL_REQUEST_URL": "https://api.github.com/repos/superkelvint/searchkernel/pulls/1",
        }
        cases = [
            ({**base, "COMMENT_BODY": "/searchkernel-verify test ruby not-a-sha"}, "exact 40-character commit SHA"),
            ({**base, "COMMENT_BODY": f"/searchkernel-verify test ruby {revision} extra"}, "usage:"),
            ({**base, "COMMENT_BODY": f"/searchkernel-verify ci ruby {revision}"}, "usage:"),
            ({**base, "COMMENT_BODY": f"/searchkernel-verify test bad/target {revision}"}, "target must be"),
        ]
        for environment, message in cases:
            with self.subTest(message=message):
                result, outputs = self.run_request(**environment)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertEqual(outputs, {})


if __name__ == "__main__":
    unittest.main()
, request)
        self.assertIn('operation not in {"test", "verify"}', text)
        self.assertIn(
            'canonical}" =~ ^[A-Za-z0-9._-]{1,128}
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
,
            text,
        )
        self.assertNotIn("eval ", text)
        self.assertNotIn("eval ", request)

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
