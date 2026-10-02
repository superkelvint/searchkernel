#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "publish-consumer-runtime.yml"
RUNNER = ROOT / "tools" / "run-in-ci-image.sh"
DEV = ROOT / "dev"
TARGETS = ROOT / "build" / "targets.json"


class ConsumerReleasePublishWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.runner = RUNNER.read_text(encoding="utf-8")
        cls.dev = DEV.read_text(encoding="utf-8")
        cls.targets = json.loads(TARGETS.read_text(encoding="utf-8"))

    def test_tag_push_publishes_but_manual_dispatch_is_dry_run(self):
        text = self.workflow
        self.assertIn('tags:\n      - "v*"', text)
        self.assertIn("workflow_dispatch:", text)
        self.assertIn("permissions:\n  contents: read\n", text)

        publish = text[text.index("  publish:") :]
        self.assertIn(
            "if: github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')",
            publish,
        )
        self.assertIn("permissions:\n      contents: write", publish)
        self.assertNotIn("contents: write", text[: text.index("  publish:")])

    def test_candidate_is_built_once_in_canonical_container_and_exported(self):
        text = self.workflow
        self.assertIn('SEARCHKERNEL_CI_EXACT_SOURCE: "1"', text)
        self.assertIn('SEARCHKERNEL_CI_EPHEMERAL_TARGET: "1"', text)
        self.assertIn("SEARCHKERNEL_CONSUMER_RELEASE_EXPORT:", text)
        self.assertIn("./dev test consumer-release", text)
        self.assertIn("actions/upload-artifact@v7", text)
        self.assertIn("actions/download-artifact@v8", text)

        self.assertIn('"consumer-release"', self.targets["test"])
        self.assertIn("package-consumer-release.py", self.dev)
        self.assertIn("check-consumer-release.py", self.dev)
        self.assertIn(
            "SEARCHKERNEL_CONSUMER_RELEASE_OUTPUT=/artifacts/consumer-release",
            self.runner,
        )
        self.assertIn(
            "exported qualified consumer release to $CONSUMER_RELEASE_EXPORT",
            self.runner,
        )

    def test_publication_revalidates_identity_and_exact_bytes(self):
        text = self.workflow
        for fragment in (
            'manifest.get("commit"',
            'entry.get("sha256") != digest',
            'runtime.get("compressed_bytes") != size',
            "transferred runtime bytes changed after qualification",
            "EXPECTED_RUNTIME_SHA256",
            "EXPECTED_RUNTIME_IDENTITY",
        ):
            self.assertIn(fragment, text)

    def test_existing_release_assets_are_immutable(self):
        publish = self.workflow[self.workflow.index("  publish:") :]
        self.assertIn("gh release download", publish)
        self.assertIn("cmp -s", publish)
        self.assertIn("already exists with different bytes", publish)
        self.assertNotIn("--clobber", publish)
        self.assertIn("--verify-tag", publish)
        self.assertIn("--prerelease", publish)

    def test_release_check_remains_separate_m5_work(self):
        self.assertIn("planned_command release-check M5", self.dev)


if __name__ == "__main__":
    unittest.main()
