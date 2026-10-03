#!/usr/bin/env python3
import pathlib
import shutil
import subprocess
import tempfile
import textwrap
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "verification-comment-cleanup.yml"


class VerificationCommentCleanupWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_runs_only_when_pull_request_becomes_ready_for_review(self):
        text = self.text
        self.assertIn("pull_request:", text)
        self.assertIn("types: [ready_for_review]", text)
        self.assertNotIn("issue_comment:", text)
        self.assertNotIn("repository_dispatch:", text)

    def test_cleanup_has_only_comment_write_permissions(self):
        text = self.text
        self.assertIn("permissions:\n  contents: read\n", text)
        job = text[text.index("  cleanup:"):]
        self.assertIn("issues: write", job)
        self.assertIn("pull-requests: write", job)
        self.assertNotIn("${{ secrets.", job)
        self.assertNotIn("actions/checkout", job)

    def test_keeps_latest_request_and_result_per_lane_and_deletes_older_pairs(self):
        text = self.text
        self.assertIn("latestRequestByLane", text)
        self.assertIn("keepRequestIds", text)
        self.assertIn("keepResultIds", text)
        self.assertIn("searchkernel-verification-request:", text)
        self.assertIn("Exact-head request:", text)
        self.assertIn("github-actions[bot]", text)
        self.assertIn("github.rest.issues.deleteComment", text)
        self.assertIn("deleteIds.add(id)", text)
        self.assertIn('requestBody.includes("<!-- searchkernel-verification-run:")', text)

    def test_cleanup_script_is_valid_javascript(self):
        text = self.text
        marker = "          script: |\n"
        script_start = text.index(marker) + len(marker)
        script = textwrap.dedent(text[script_start:])
        node = shutil.which("node")
        self.assertIsNotNone(node, "node is required to syntax-check the cleanup github-script")
        with tempfile.TemporaryDirectory(prefix="searchkernel-comment-cleanup.") as raw_tmp:
            path = pathlib.Path(raw_tmp) / "cleanup.mjs"
            path.write_text(script, encoding="utf-8")
            result = subprocess.run(
                [node, "--check", str(path)],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
