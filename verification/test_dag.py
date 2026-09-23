#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import dag as verification_dag

ROOT = Path(__file__).resolve().parents[1]
DAG_PATH = ROOT / "verification" / "dag.toml"


class VerificationDagTests(unittest.TestCase):
    def test_current_repository_is_structurally_valid_and_covered(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        verification_dag.validate_repository_coverage(current, ROOT)

    def test_every_current_web_feature_family_has_subsystem_ownership(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        features = ROOT / "searchkernel-web" / "src" / "features"
        for child in sorted(path for path in features.iterdir() if path.is_dir()):
            probe = f"searchkernel-web/src/features/{child.name}/__coverage_probe__.tsx"
            with self.subTest(feature=child.name):
                subsystems = verification_dag.matching_subsystems(current, probe)
                self.assertTrue(
                    any(subsystem.behavioral for subsystem in subsystems),
                    f"{child.name} has no behavioral DAG owner",
                )

    def test_new_core_crate_is_not_silently_owned(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        probe = "core/crates/searchkernel-new/src/lib.rs"
        self.assertFalse(verification_dag.matching_subsystems(current, probe))
        self.assertFalse(verification_dag.matching_exemptions(current, probe))

    def test_new_web_feature_family_is_not_silently_owned(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        probe = "searchkernel-web/src/features/new-capability/Page.tsx"
        self.assertFalse(verification_dag.matching_subsystems(current, probe))
        self.assertFalse(verification_dag.matching_exemptions(current, probe))

    def test_ruby_client_family_has_explicit_verification_ownership(self) -> None:
        current = verification_dag.load_dag(DAG_PATH)
        probe = "clients/ruby/lib/searchkernel.rb"
        owners = verification_dag.matching_subsystems(current, probe)
        self.assertEqual([owner.id for owner in owners], ["client-ruby"])
        self.assertEqual(owners[0].requires, ("ruby-client", "client-contracts"))
        self.assertIn("ruby-client", verification_dag.task_closure(current, ("full",)))

    def test_unknown_task_reference_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        mutated = source.replace(
            'requires = ["rust-non-native", "rust-quality"]',
            'requires = ["rust-non-native", "does-not-exist"]',
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(mutated, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "unknown required task"):
                verification_dag.load_dag(path)

    def test_duplicate_task_id_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[task]]
id = "rust-non-native"
kind = "aggregate"
runner = "linux"
depends_on = []
cache_result = false
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "duplicate task id"):
                verification_dag.load_dag(path)

    def test_behavioral_subsystem_task_must_be_reachable_from_full_root(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source += """
[[task]]
id = "orphan-behavior"
kind = "command"
runner = "linux"
command = "true"
depends_on = []
cache_result = false

[[subsystem]]
id = "orphan-subsystem"
paths = ["orphan/**"]
requires = ["orphan-behavior"]
"""
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(
                verification_dag.DagError,
                "not reachable from full_roots",
            ):
                verification_dag.load_dag(path)

    def test_dependency_cycle_is_rejected(self) -> None:
        source = DAG_PATH.read_text(encoding="utf-8")
        source = source.replace(
            'id = "rust-quality"\nlegacy_lanes = ["rust"]\nkind = "command"\nrunner = "linux"\ncommand = "cargo fmt --all --check && cargo clippy --workspace --all-targets --no-default-features -- -D warnings"\ndepends_on = []',
            'id = "rust-quality"\nlegacy_lanes = ["rust"]\nkind = "command"\nrunner = "linux"\ncommand = "cargo fmt --all --check && cargo clippy --workspace --all-targets --no-default-features -- -D warnings"\ndepends_on = ["rust-non-native"]',
            1,
        )
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "dag.toml"
            path.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(verification_dag.DagError, "dependency cycle"):
                verification_dag.load_dag(path)


if __name__ == "__main__":
    unittest.main()
