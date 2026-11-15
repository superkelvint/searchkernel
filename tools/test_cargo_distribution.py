#!/usr/bin/env python3
"""Regression coverage for the Cargo artifact preparation boundary."""
import importlib.util
import json
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("cargo_packager", ROOT / "tools/package-consumer-release.py")
PACKAGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGER)


class CargoDistributionTests(unittest.TestCase):
    def setUp(self):
        patcher = mock.patch.object(PACKAGER, "cargo_tool", return_value="cargo")
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_package_failure_restores_checkout_and_never_publishes_descriptor(self):
        bindings = ROOT / "core/crates/searchkernel-vespa-bindings"
        original = (bindings / "consumer_bundle.rs").read_bytes()
        with tempfile.TemporaryDirectory() as raw:
            with mock.patch.object(PACKAGER.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")):
                with self.assertRaises(subprocess.CalledProcessError):
                    PACKAGER.build_rust_crates(Path(raw), {"runtime_identity": "test"})
        self.assertFalse((bindings / "runtime.json").exists())
        self.assertEqual((bindings / "consumer_bundle.rs").read_bytes(), original)

    def test_actual_cargo_packager_receives_exact_runtime_descriptor(self):
        bindings = ROOT / "core/crates/searchkernel-vespa-bindings"
        descriptor = {"runtime_identity": "test-exact-identity", "artifact_sha256": "e" * 64}

        def cargo_package(command, **kwargs):
            staged_bindings = kwargs["cwd"] / "core/crates/searchkernel-vespa-bindings"
            self.assertEqual(command[1], "package")
            self.assertIn("--workspace", command)
            self.assertIn("--locked", command)
            self.assertNotEqual(staged_bindings, bindings)
            self.assertEqual(json.loads((staged_bindings / "runtime.json").read_text()), descriptor)
            # Release crates must not embed the development/native-base source bundle.
            self.assertIn("= &[];", (staged_bindings / "consumer_bundle.rs").read_text())
            target = Path(command[command.index("--target-dir") + 1]) / "package"
            target.mkdir()
            with tarfile.open(target / "searchkernel-0.1.0.crate", "w:gz") as archive:
                archive.add(staged_bindings / "runtime.json", arcname="searchkernel-0.1.0/runtime.json")

        with tempfile.TemporaryDirectory() as raw:
            with mock.patch.object(PACKAGER.subprocess, "run", side_effect=cargo_package):
                crates = PACKAGER.build_rust_crates(Path(raw), descriptor)
            self.assertEqual([path.name for path in crates], ["searchkernel-0.1.0.crate"])
            with tarfile.open(crates[0]) as archive:
                self.assertEqual(json.load(archive.extractfile("searchkernel-0.1.0/runtime.json")), descriptor)
        self.assertFalse((bindings / "runtime.json").exists())


if __name__ == "__main__":
    unittest.main()
