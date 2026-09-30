#!/usr/bin/env python3
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class QualificationPolicyTests(unittest.TestCase):
    def test_gate_runs_in_consumer_container_without_producer_state(self):
        runner = (ROOT / ".github/ci/ci-image-runner.yml").read_text()
        marker = 'echo "==> Qualify packaged release in clean consumer"'
        self.assertIn(marker, runner)
        section = runner.split(marker, 1)[1].split("\n          fi", 1)[0]
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release:/release:ro"', section)
        self.assertIn('-v "$ARTIFACT_DIR/consumer-release-tools:/qualification:ro"', section)
        self.assertIn("python3 /qualification/qualify-consumer-release.py /release", section)
        for forbidden in (
            "$SOURCE_VOLUME",
            "$TARGET_VOLUME",
            "$NATIVE_VOLUME",
            "/work",
            "/cargo-target",
            "/opt/searchkernel/native-sdk-cache",
        ):
            self.assertNotIn(forbidden, section)

    def test_producer_exports_exact_release_artifacts_for_consumer_gate(self):
        runner = (ROOT / ".github/ci/ci-image-runner.yml").read_text()
        self.assertIn(
            "tools/package-consumer-release.py --output /artifacts/consumer-release",
            runner,
        )
        self.assertIn(
            "tools/check-consumer-release.py /artifacts/consumer-release",
            runner,
        )
        self.assertIn(
            "cp tools/qualify-consumer-release.py /artifacts/consumer-release-tools/",
            runner,
        )

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

    def test_packaged_supported_rows_own_clean_consumer_gate(self):
        authority = json.loads((ROOT / "docs/release-support.json").read_text())
        packaged = {"rust-embedded", "searchkerneld", "python", "typescript-node"}
        rows = [row for row in authority["supported"] if row["surface"] in packaged]
        self.assertTrue(rows)
        self.assertEqual(
            {"tools/qualify-consumer-release.py dist/release"},
            {row.get("qualification") for row in rows},
        )
        ruby = [row for row in authority["supported"] if row["surface"] == "ruby"]
        self.assertTrue(ruby, "Ruby connect release support must remain intact")


if __name__ == "__main__":
    unittest.main()
