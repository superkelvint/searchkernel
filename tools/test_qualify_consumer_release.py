#!/usr/bin/env python3
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class QualificationPolicyTests(unittest.TestCase):
    def test_gate_runs_in_consumer_container_without_producer_state(self):
        workflow = (ROOT / ".github/workflows/ci-image-runner.yml").read_text()
        marker = "- name: Qualify packaged release in clean consumer"
        self.assertIn(marker, workflow)
        section = workflow.split(marker, 1)[1].split("\n      - name:", 1)[0]
        self.assertIn(
            '-v "$ARTIFACT_DIR/consumer-release:/release:ro"',
            section,
        )
        self.assertIn(
            '-v "$ARTIFACT_DIR/consumer-release-tools:/qualification:ro"',
            section,
        )
        for forbidden in (
            "$SOURCE_VOLUME",
            "$TARGET_VOLUME",
            "$NATIVE_VOLUME",
            "/work",
            "/cargo-target",
            "/opt/searchkernel/native-sdk-cache",
        ):
            self.assertNotIn(forbidden, section)
        self.assertIn(
            '"$PYTHON" /qualification/qualify-consumer-release.py /release',
            section,
        )

    def test_producer_exports_exact_release_artifacts_for_consumer_gate(self):
        workflow = (ROOT / ".github/workflows/ci-image-runner.yml").read_text()
        self.assertIn(
            "tools/package-consumer-release.py --output /artifacts/consumer-release",
            workflow,
        )
        self.assertIn(
            "tools/check-consumer-release.py /artifacts/consumer-release",
            workflow,
        )
        self.assertIn(
            "cp tools/qualify-consumer-release.py /artifacts/consumer-release-tools/",
            workflow,
        )
        producer = workflow.split("            client-e2e)", 1)[1].split(
            "              ;;", 1
        )[0]
        self.assertNotIn("client-release-qualify", producer)

    def test_release_changes_trigger_client_e2e_owner(self):
        workflow = (ROOT / ".github/workflows/client-capabilities.yml").read_text()
        for required in (
            "tools/package-consumer-release.py",
            "tools/check-consumer-release.py",
            "tools/check-release-support.py",
            "tools/qualify-consumer-release.py",
            "docs/release-support.json",
            ".github/workflows/ci-image-runner.yml",
        ):
            self.assertGreaterEqual(
                workflow.count(f'- "{required}"'),
                2,
                f"{required} must trigger client-e2e on PRs and main pushes",
            )
        self.assertIn("target: client-e2e", workflow)

    def test_gate_scrubs_repository_and_native_runtime_overrides(self):
        gate = (ROOT / "tools/qualify-consumer-release.py").read_text()
        self.assertIn('key.startswith(("SEARCHKERNEL_", "SK_NATIVE_", "VESPA_"))', gate)
        for variable in (
            "CARGO_MANIFEST_DIR",
            "CARGO_TARGET_DIR",
            "LD_LIBRARY_PATH",
            "LIBRARY_PATH",
            "NODE_PATH",
            "PYTHONPATH",
        ):
            self.assertIn(variable, gate)
        self.assertNotIn('"--no-index"', gate)

    def test_all_supported_rows_own_same_packaged_gate(self):
        authority = json.loads((ROOT / "docs/release-support.json").read_text())
        self.assertTrue(authority["supported"])
        self.assertEqual(
            {"tools/qualify-consumer-release.py dist/release"},
            {row.get("qualification") for row in authority["supported"]},
        )


if __name__ == "__main__":
    unittest.main()
