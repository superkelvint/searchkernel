#!/usr/bin/env python3
"""Tests for generated first-party client audit coverage."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check-audit-client-coverage.py")
SPEC = importlib.util.spec_from_file_location("check_audit_client_coverage", MODULE_PATH)
assert SPEC and SPEC.loader
client_coverage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(client_coverage)


def fixture() -> tuple[dict, dict, list[str], str]:
    catalog = {
        "surfaces": {
            "core": {},
            "rust": {},
            "php": {
                "evidence_roots": ["clients/php/tests"],
                "e2e_lanes": [{"path": "clients/php/examples/e2e.php"}],
            },
            "ruby": {
                "evidence_roots": ["clients/ruby/test"],
                "e2e_lanes": [{"path": "clients/ruby/examples/e2e.rb"}],
            },
        }
    }
    modules = {
        "php": {
            "module": "28-client-php.md",
            "production_roots": ["clients/php/src", "clients/php/composer.json"],
            "support_roots": ["clients/php/tests", "clients/php/examples"],
        },
        "ruby": {
            "module": "29-client-ruby.md",
            "production_roots": ["clients/ruby/lib", "clients/ruby/searchkernel.gemspec"],
            "support_roots": ["clients/ruby/test", "clients/ruby/examples"],
        },
    }
    manifest = {
        "client_audit_modules": modules,
        "files": [{"file": "28-client-php.md"}, {"file": "29-client-ruby.md"}],
    }
    files = [
        "clients/php/src/Client.php",
        "clients/php/tests/run.php",
        "clients/php/examples/e2e.php",
        "clients/php/composer.json",
        "clients/ruby/lib/client.rb",
        "clients/ruby/test/run.rb",
        "clients/ruby/examples/e2e.rb",
        "clients/ruby/searchkernel.gemspec",
    ]
    readme = "| PHP client | [28-client-php.md](28-client-php.md) |\n| Ruby client | [29-client-ruby.md](29-client-ruby.md) |\n"
    return catalog, manifest, files, readme


class ClientAuditCoverageTests(unittest.TestCase):
    def test_current_client_files_map_to_dedicated_modules(self) -> None:
        catalog, manifest, files, readme = fixture()
        inventory, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertEqual([], errors)
        self.assertEqual(8, len(inventory))

    def test_unclassified_client_file_fails_closed(self) -> None:
        catalog, manifest, files, readme = fixture()
        files.append("clients/php/new-runtime-file.php")
        _, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertTrue(any("maps to 0 audit inventory roots" in error for error in errors))

    def test_new_first_party_catalog_surface_requires_mapping(self) -> None:
        catalog, manifest, files, readme = fixture()
        catalog["surfaces"]["elixir"] = {"evidence_roots": ["clients/elixir/test"], "e2e_lanes": []}
        _, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertTrue(any("lack audit-module mappings: elixir" in error for error in errors))

    def test_e2e_lane_must_be_covered_as_audit_support(self) -> None:
        catalog, manifest, files, readme = fixture()
        catalog["surfaces"]["php"]["e2e_lanes"] = [{"path": "tools/php-e2e.php"}]
        _, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertTrue(any("E2E lane is outside its package root" in error for error in errors))


    def test_production_and_support_roots_are_both_required(self) -> None:
        catalog, manifest, files, readme = fixture()
        manifest["client_audit_modules"]["php"]["production_roots"] = []
        _, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertTrue(any("non-empty production_roots" in error for error in errors))

    def test_declared_inventory_root_must_cover_a_tracked_file(self) -> None:
        catalog, manifest, files, readme = fixture()
        manifest["client_audit_modules"]["php"]["production_roots"].append("clients/php/src-unused")
        _, errors = client_coverage.derive_inventory(catalog, manifest, files, readme)
        self.assertTrue(any("root has no tracked files" in error for error in errors))

if __name__ == "__main__":
    unittest.main()
