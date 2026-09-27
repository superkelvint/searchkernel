#!/usr/bin/env python3
import importlib.util
import os
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
CLIENT_CAPABILITIES_WORKFLOW = ROOT / ".github" / "workflows" / "client-capabilities.yml"
RELEASE_VERIFIER = ROOT / "tools" / "check-consumer-release.py"
SPEC = importlib.util.spec_from_file_location(
    "package_consumer_release", ROOT / "tools" / "package-consumer-release.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ConsumerReleaseTests(unittest.TestCase):
    def test_daemon_path_uses_cargo_target_dir_override(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            target = Path(tempdir) / "cargo-target"
            with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target)}):
                self.assertEqual(
                    MODULE.daemon_binary(), target / "release" / "searchkerneld"
                )

    def test_daemon_path_defaults_to_workspace_target_dir(self) -> None:
        with mock.patch.dict(os.environ):
            os.environ.pop("CARGO_TARGET_DIR", None)
            self.assertEqual(
                MODULE.daemon_binary(), ROOT / "target" / "release" / "searchkerneld"
            )

    def test_rust_bundle_is_self_contained_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            bundle = Path(tempdir) / "searchkernel-rust-test.tar.gz"
            MODULE.build_rust_bundle(bundle)

            with tarfile.open(bundle) as archive:
                names = set(archive.getnames())
                self.assertIn("LICENSE", names)
                self.assertIn("core/crates/searchkernel/Cargo.toml", names)
                self.assertIn("core/crates/searchkernel-rpc/Cargo.toml", names)
                self.assertIn("searchkerneld/Cargo.toml", names)
                self.assertIn(
                    "core/crates/searchkernel-vespa-bindings/consumer_bundle.rs", names
                )
                self.assertNotIn("tools/resolve-native-sdk.sh", names)
                extracted = Path(tempdir) / "extracted"
                archive.extractall(extracted)

            subprocess.run(
                [
                    "cargo",
                    "metadata",
                    "--no-deps",
                    "--format-version",
                    "1",
                    "--manifest-path",
                    str(extracted / "Cargo.toml"),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )


    def test_client_capability_workflow_watches_every_rust_bundle_input(self) -> None:
        workflow = CLIENT_CAPABILITIES_WORKFLOW.read_text(encoding="utf-8")
        for relative in MODULE.RUST_BUNDLE_PATHS:
            path = ROOT / relative
            watch = f"{relative}/**" if path.is_dir() else relative
            with self.subTest(watch=watch):
                self.assertEqual(
                    workflow.count(f'      - "{watch}"'),
                    2,
                    f"{watch} must trigger both pull_request and main push release qualification",
                )

    def test_release_verifier_builds_extracted_native_searchkernel(self) -> None:
        verifier = RELEASE_VERIFIER.read_text(encoding="utf-8")
        self.assertIn('"check", "-p", "searchkernel", "--features", "native"', verifier)
        self.assertIn('"CARGO_TARGET_DIR"', verifier)
        self.assertIn('environment.pop("SK_NATIVE_SDK", None)', verifier)
        self.assertIn('environment.pop("SK_NATIVE_ABI_HEADER", None)', verifier)



if __name__ == "__main__":
    unittest.main()
