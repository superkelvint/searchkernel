#!/usr/bin/env python3
"""Regression tests for fail-closed Cargo target ownership checks."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tools/check-verifier-build-ownership.py"
SPEC = importlib.util.spec_from_file_location("check_verifier_build_ownership", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def metadata(*targets: tuple[str, str, str]) -> dict[str, object]:
    packages: dict[str, dict[str, object]] = {}
    for package, name, source in targets:
        package_value = packages.setdefault(
            package,
            {"name": package, "targets": []},
        )
        package_value["targets"].append(
            {"name": name, "kind": ["example"], "src_path": source}
        )
    return {"packages": list(packages.values())}


class VerifierBuildOwnershipTests(unittest.TestCase):
    def test_nonexistent_example_name_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            script = root / "reference/vespa/m8/verify.sh"
            script.parent.mkdir(parents=True)
            script.write_text("cargo run -p owner --example missing\n", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "not registered"):
                MODULE.check_example_routing(root, metadata(("owner", "real", "/tmp/real.rs")))

    def test_valid_example_with_wrong_package_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            script = root / "reference/vespa/m8/verify.sh"
            script.parent.mkdir(parents=True)
            script.write_text("cargo run -p wrong --example real\n", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "not registered"):
                MODULE.check_example_routing(root, metadata(("owner", "real", "/tmp/real.rs")))

    def test_second_cargo_command_on_same_line_is_checked(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            script = root / "reference/vespa/m8/verify.sh"
            script.parent.mkdir(parents=True)
            script.write_text(
                "cargo run -p owner --example real && cargo run -p owner --example missing\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(SystemExit, "missing"):
                MODULE.check_example_routing(root, metadata(("owner", "real", "/tmp/real.rs")))

    def test_existing_rs_file_without_registered_target_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            source = root / "core/crates/owner/examples/ghost.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn main() {}\n", encoding="utf-8")
            spec = MODULE.target_spec(
                "owner",
                "ghost",
                "core/crates/owner/examples/ghost.rs",
            )
            with self.assertRaisesRegex(SystemExit, "not registered"):
                MODULE.check_cargo_target_ownership(
                    root,
                    metadata(),
                    specs=[spec],
                    sources=("core/crates/owner/examples/ghost.rs",),
                )

    def test_retired_standalone_manifest_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            manifest = root / "reference/vespa/m8/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text("[package]\nname = \"retired\"\n", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "standalone Cargo manifests remain"):
                MODULE.check_manifests_and_runners(root)


if __name__ == "__main__":
    unittest.main()
