#!/usr/bin/env python3
"""Regression tests for check-native-abi-authorities.py."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER_PATH = ROOT / "tools" / "check-native-abi-authorities.py"
SPEC = importlib.util.spec_from_file_location("check_native_abi_authorities", CHECKER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"could not load {CHECKER_PATH}")
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class NativeAbiAuthorityTests(unittest.TestCase):
    def write_fixture(self, root: Path, version: int = 13) -> None:
        files = {
            "core/native/vespa/searchkernel_vespa.h":
                f"#define SK_NATIVE_ABI_VERSION {version}U\n",
            "core/native/vespa/manifest.json":
                json.dumps(
                    {
                        "native_abi_version": version,
                        "vespa_commit": "deadbeef",
                    }
                )
                + "\n",
            "core/crates/searchkernel-vespa-bindings/src/abi.rs":
                f"pub const EXPECTED_ABI: u32 = {version};\n",
            "core/crates/searchkernel-vespa-bindings/src/bindings.rs":
                f"pub const SK_NATIVE_ABI_VERSION: u32 = {version};\n",
            "reference/vespa/NATIVE_ABI_CONTRACT.h":
                f"#define SK_NATIVE_ABI_VERSION {version}U\n",
            "reference/vespa/PINNED_VESPA.json":
                json.dumps(
                    {
                        "commit": "deadbeef",
                        "native_abi_version": version,
                    }
                )
                + "\n",
            "reference/vespa/m0/expected/01-native-open-close.json":
                json.dumps(
                    {
                        "scenario": "native-open-close",
                        "abi_version": version,
                        "open_count": 2,
                        "status": "ok",
                    }
                )
                + "\n",
            "reference/vespa/m0/scenarios/01-native-open-close.json":
                json.dumps(
                    {
                        "scenario": "native-open-close",
                        "opens": 2,
                        "same_data_directory": True,
                        "native_abi_version": version,
                    }
                )
                + "\n",
            "reference/vespa/phase9/expected/01-additive-handles.json":
                json.dumps(
                    {
                        "id": "additive-handles",
                        "status": "ok",
                        "abi": version,
                    }
                )
                + "\n",
        }
        for relative, contents in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents, encoding="utf-8")

    def authorities(self, root: Path, *, git_index: bool = False) -> dict[str, int]:
        authorities, _ = CHECKER.repository_authorities(
            CHECKER.RepositoryReader(root, git_index=git_index)
        )
        return authorities

    def test_matching_repository_authorities_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            self.assertIsNone(CHECKER.disagreement(self.authorities(root)))

    def test_reference_contract_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            contract = root / "reference/vespa/NATIVE_ABI_CONTRACT.h"
            contract.write_text("#define SK_NATIVE_ABI_VERSION 12U\n", encoding="utf-8")
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("reference ABI contract=12", mismatch)

    def test_pinned_metadata_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            pin = root / "reference/vespa/PINNED_VESPA.json"
            pin.write_text(
                json.dumps({"commit": "deadbeef", "native_abi_version": 12}) + "\n",
                encoding="utf-8",
            )
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("pinned metadata=12", mismatch)

    def test_native_manifest_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            manifest = root / "core/native/vespa/manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "native_abi_version": 12,
                        "vespa_commit": "deadbeef",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("native manifest=12", mismatch)

    def test_m0_expected_abi_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            expected = root / "reference/vespa/m0/expected/01-native-open-close.json"
            expected.write_text(
                json.dumps(
                    {
                        "scenario": "native-open-close",
                        "abi_version": 12,
                        "open_count": 2,
                        "status": "ok",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("M0 native-open-close expected=12", mismatch)

    def test_other_verifier_abi_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            scenario = root / "reference/vespa/m0/scenarios/01-native-open-close.json"
            scenario.write_text(
                json.dumps(
                    {
                        "scenario": "native-open-close",
                        "opens": 2,
                        "same_data_directory": True,
                        "native_abi_version": 12,
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("M0 native-open-close scenario=12", mismatch)

            self.write_fixture(root)
            phase9 = root / "reference/vespa/phase9/expected/01-additive-handles.json"
            phase9.write_text(
                json.dumps({"id": "additive-handles", "status": "ok", "abi": 12})
                + "\n",
                encoding="utf-8",
            )
            mismatch = CHECKER.disagreement(self.authorities(root))
            self.assertIsNotNone(mismatch)
            self.assertIn("Phase 9 additive expected=12", mismatch)

    def test_native_manifest_vespa_commit_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            manifest = root / "core/native/vespa/manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "native_abi_version": 13,
                        "vespa_commit": "wrong",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            with self.assertRaises(CHECKER.AuthorityCheckError):
                self.authorities(root)

    def test_sdk_authorities_and_pin_are_checked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            authorities, pin = CHECKER.repository_authorities(
                CHECKER.RepositoryReader(root)
            )
            sdk_header = root / "sdk/include/searchkernel_vespa.h"
            sdk_manifest = root / "sdk/manifest.json"
            sdk_header.parent.mkdir(parents=True)
            sdk_header.write_text(
                "#define SK_NATIVE_ABI_VERSION 13U\n", encoding="utf-8"
            )
            sdk_manifest.write_text(
                json.dumps(
                    {
                        "native_abi_version": 13,
                        "vespa_commit": "deadbeef",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            CHECKER.add_sdk_authorities(
                authorities,
                sdk_header=sdk_header,
                sdk_manifest=sdk_manifest,
                pinned_commit=pin,
            )
            self.assertIsNone(CHECKER.disagreement(authorities))

            sdk_manifest.write_text(
                json.dumps(
                    {
                        "native_abi_version": 12,
                        "vespa_commit": "deadbeef",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            sdk_version_mismatch = dict(self.authorities(root))
            CHECKER.add_sdk_authorities(
                sdk_version_mismatch,
                sdk_header=sdk_header,
                sdk_manifest=sdk_manifest,
                pinned_commit=pin,
            )
            mismatch = CHECKER.disagreement(sdk_version_mismatch)
            self.assertIsNotNone(mismatch)
            self.assertIn("SDK manifest=12", mismatch)

            sdk_manifest.write_text(
                json.dumps(
                    {
                        "native_abi_version": 13,
                        "vespa_commit": "wrong",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            with self.assertRaises(CHECKER.AuthorityCheckError):
                CHECKER.add_sdk_authorities(
                    dict(self.authorities(root)),
                    sdk_header=sdk_header,
                    sdk_manifest=sdk_manifest,
                    pinned_commit=pin,
                )

    def test_git_index_mode_checks_staged_versions_not_worktree_versions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(
                ["git", "config", "user.name", "ABI Test"], cwd=root, check=True
            )
            subprocess.run(
                ["git", "config", "user.email", "abi@example.invalid"],
                cwd=root,
                check=True,
            )
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(
                ["git", "commit", "-q", "-m", "fixture"], cwd=root, check=True
            )

            source = root / "core/native/vespa/searchkernel_vespa.h"
            source.write_text(
                "#define SK_NATIVE_ABI_VERSION 14U\n", encoding="utf-8"
            )
            subprocess.run(
                ["git", "add", "core/native/vespa/searchkernel_vespa.h"],
                cwd=root,
                check=True,
            )
            # Make the working tree look healthy again. The staged commit is
            # still wrong and must remain the authority for pre-commit.
            source.write_text(
                "#define SK_NATIVE_ABI_VERSION 13U\n", encoding="utf-8"
            )

            mismatch = CHECKER.disagreement(
                self.authorities(root, git_index=True)
            )
            self.assertIsNotNone(mismatch)
            self.assertIn("source header=14", mismatch)
            self.assertIn("native manifest=13", mismatch)


if __name__ == "__main__":
    unittest.main()
