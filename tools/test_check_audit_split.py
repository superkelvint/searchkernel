#!/usr/bin/env python3
"""Tests for the audit checklist split/manifest verifier."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("check-audit-split.py")
SPEC = importlib.util.spec_from_file_location("check_audit_split", MODULE_PATH)
assert SPEC and SPEC.loader
audit_split = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit_split)


class AuditSplitTests(unittest.TestCase):
    def baseline(self, *ids: str) -> dict:
        return {
            "source_item_count": len(ids),
            "source_item_ids_sha256": audit_split._ids_digest(set(ids)),
        }

    def test_generated_manifest_passes_with_unique_ids_and_ranges(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "28-client-php.md").write_text(
                "- [ ] **SK-28-001** first\n- [ ] **SK-28-002** second\n", encoding="utf-8"
            )
            (audit_dir / "README.md").write_text("index\n", encoding="utf-8")
            manifest = audit_split.update_manifest(
                audit_dir,
                {
                    **self.baseline(),
                    "live_extensions": [
                        {
                            "trigger": "test extension",
                            "item_ranges": ["SK-28-001..SK-28-002"],
                            "added_item_count": 2,
                        }
                    ],
                },
            )
            self.assertEqual([], audit_split.validate_manifest(audit_dir, manifest))

    def test_duplicate_ids_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "a.md").write_text("- [ ] **SK-01-001** one\n", encoding="utf-8")
            (audit_dir / "b.md").write_text("- [ ] **SK-01-001** duplicate\n", encoding="utf-8")
            manifest = audit_split.update_manifest(audit_dir, {**self.baseline("SK-01-001"), "live_extensions": []})
            errors = audit_split.validate_manifest(audit_dir, manifest)
            self.assertTrue(any("duplicate checklist IDs" in error for error in errors))

    def test_stale_hash_or_count_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "a.md").write_text("- [ ] **SK-01-001** one\n", encoding="utf-8")
            manifest = audit_split.update_manifest(audit_dir, {**self.baseline("SK-01-001"), "live_extensions": []})
            manifest["files"][0]["sha256"] = "stale"
            errors = audit_split.validate_manifest(audit_dir, manifest)
            self.assertTrue(any("manifest inventory stale" in error for error in errors))

    def test_overlapping_live_extension_ranges_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "a.md").write_text(
                "- [ ] **SK-01-001** one\n- [ ] **SK-01-002** two\n", encoding="utf-8"
            )
            manifest = audit_split.update_manifest(
                audit_dir,
                {
                    **self.baseline(),
                    "live_extensions": [
                        {"trigger": "first", "item_ranges": ["SK-01-001..SK-01-001"], "added_item_count": 1},
                        {"trigger": "second", "item_ranges": ["SK-01-001..SK-01-002"], "added_item_count": 2},
                    ]
                },
            )
            errors = audit_split.validate_manifest(audit_dir, manifest)
            self.assertTrue(any("declared by multiple extensions" in error for error in errors))


    def test_undeclared_new_id_fails_source_identity_check(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "a.md").write_text(
                "- [ ] **SK-01-001** original\n- [ ] **SK-01-002** undeclared\n",
                encoding="utf-8",
            )
            manifest = audit_split.update_manifest(
                audit_dir, {**self.baseline("SK-01-001"), "live_extensions": []}
            )
            errors = audit_split.validate_manifest(audit_dir, manifest)
            self.assertTrue(any("baseline checklist contains" in error for error in errors))

    def test_replacing_baseline_id_with_undeclared_id_fails_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            audit_dir = Path(temp)
            (audit_dir / "a.md").write_text(
                "- [ ] **SK-01-002** replacement\n", encoding="utf-8"
            )
            manifest = audit_split.update_manifest(
                audit_dir, {**self.baseline("SK-01-001"), "live_extensions": []}
            )
            errors = audit_split.validate_manifest(audit_dir, manifest)
            self.assertTrue(any("differs from the frozen source split" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
