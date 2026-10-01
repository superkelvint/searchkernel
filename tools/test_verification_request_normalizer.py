#!/usr/bin/env python3
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
NORMALIZER = ROOT / "tools" / "normalize-verification-request.sh"


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
                ["bash", str(NORMALIZER)],
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

    def test_repository_dispatch_preserves_existing_request_shape(self):
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
