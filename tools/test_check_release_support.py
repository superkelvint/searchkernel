#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / "tools" / "check-release-support.py"

spec = importlib.util.spec_from_file_location("check_release_support", TOOL)
MODULE = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(MODULE)


class ReleaseSupportCheckerTest(unittest.TestCase):
    def test_registered_dev_target_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "dev").write_text("#!/bin/sh\n", encoding="utf-8")
            registry = {"test": {"ruby": {}}, "verify": {"persistence": {}}}
            MODULE.validate_verification_command(root, "./dev test ruby", registry)
            MODULE.validate_verification_command(root, "./dev verify persistence", registry)

    def test_unregistered_dev_target_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "dev").write_text("#!/bin/sh\n", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "unregistered test target"):
                MODULE.validate_verification_command(
                    root, "./dev test imaginary", {"test": {}, "verify": {}}
                )

    def test_release_validation_does_not_depend_on_retired_ci_yaml(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "clients").mkdir()
            (root / "build").mkdir()
            (root / "tools").mkdir()
            (root / "dev").write_text("#!/bin/sh\n", encoding="utf-8")
            shutil.copy2(ROOT / "tools" / "qualify-consumer-release.py", root / "tools")

            supported = [
                {
                    "surface": surface,
                    "runtime": runtime,
                    "evidence": evidence,
                    "verification": MODULE.REQUIRED_VERIFICATION[(surface, runtime)],
                    "qualification": MODULE.PACKAGED_QUALIFICATION,
                }
                for surface, runtime, evidence in sorted(MODULE.EXPECTED_SUPPORTED)
            ]
            (root / "docs" / "release-support.json").write_text(
                json.dumps(
                    {
                        "platform": {"os": "linux", "arch": "x86_64", "libc": "glibc"},
                        "supported": supported,
                        "deferred": sorted(MODULE.REQUIRED_DEFERRED),
                    }
                ),
                encoding="utf-8",
            )
            (root / "build" / "targets.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "test": {"python": {}, "typescript": {}, "ruby": {}},
                        "verify": {"persistence": {}, "product.integration": {}},
                    }
                ),
                encoding="utf-8",
            )
            (root / "README.md").write_text(
                "Ruby (managed-local/connect-to-daemon)\n[release](docs/release-support.md)\n",
                encoding="utf-8",
            )
            (root / "clients" / "README.md").write_text(
                "Ruby (managed-local/connect-to-daemon)\n[release](../docs/release-support.md)\n",
                encoding="utf-8",
            )
            (root / "docs" / "release-support.md").write_text(
                "| Ruby | Supported | managed-local and connect-to-daemon |\n",
                encoding="utf-8",
            )

            # Deliberately do not create .github/ci/ci-image-runner.yml.
            MODULE.validate(root)


if __name__ == "__main__":
    unittest.main()
