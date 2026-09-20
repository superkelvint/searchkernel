#!/usr/bin/env python3
"""Tests for the capability ledger validator and document generator."""

from __future__ import annotations

import copy
import importlib.util
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "check-capabilities.py"
SPEC = importlib.util.spec_from_file_location("check_capabilities", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CapabilityLedgerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.ledger = MODULE.load_ledger()

    def test_committed_ledger_and_documents_are_valid(self) -> None:
        documents = MODULE.generated_documents(self.ledger)
        self.assertEqual(MODULE.check_documents(documents), [])

    def test_duplicate_ids_are_rejected(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        ledger["capabilities"].append(copy.deepcopy(ledger["capabilities"][0]))
        with self.assertRaisesRegex(MODULE.LedgerError, "duplicate capability id"):
            MODULE.validate_ledger(ledger)

    def test_missing_evidence_path_is_rejected(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        ledger["capabilities"][0]["verified_by"] = ["does-not-exist.md"]
        with self.assertRaisesRegex(MODULE.LedgerError, "does not exist"):
            MODULE.validate_ledger(ledger)

    def test_stale_document_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "generated.md"
            path.write_text("stale\n", encoding="utf-8")
            self.assertEqual(MODULE.check_documents({path: "fresh\n"}), [str(path)])

    def test_rendering_is_deterministic(self) -> None:
        first = MODULE.generated_documents(self.ledger)
        second = MODULE.generated_documents(self.ledger)
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
