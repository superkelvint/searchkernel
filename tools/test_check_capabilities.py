#!/usr/bin/env python3
"""Tests for the capability ledger validator and document generator."""

from __future__ import annotations

import copy
import importlib.util
import re
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

    def test_generated_capability_document_points_to_client_evidence(self) -> None:
        document = MODULE.render_capabilities(
            self.ledger,
            MODULE.validate_ledger(self.ledger),
        )
        self.assertIn("CLIENT_CAPABILITIES.md", document)
        self.assertNotIn("| TypeScript |", document)

    def test_query_term_evidence_uses_split_adapter_query_tests(self) -> None:
        capability = next(
            capability
            for capability in self.ledger["capabilities"]
            if capability["id"] == "query.term"
        )
        self.assertIn(
            "core/crates/searchkernel-ir-adapter/src/tests/query_lowering.rs",
            capability["verified_by"],
        )

    def test_generated_documents_include_structured_facade_state(self) -> None:
        documents = MODULE.generated_documents(self.ledger)
        self.assertIn(
            "Structured facade support",
            documents[ROOT / "docs" / "CAPABILITIES.md"],
        )
        self.assertIn(
            "`query.phrase` | typescript | supported",
            documents[ROOT / "docs" / "CAPABILITIES.md"],
        )
        self.assertNotIn(
            "machine-checked structured support state",
            documents[ROOT / "LIMITATIONS.md"],
        )

    def test_query_term_evidence_uses_split_adapter_query_tests(self) -> None:
        capability = next(
            capability
            for capability in self.ledger["capabilities"]
            if capability["id"] == "query.term"
        )
        self.assertIn(
            "core/crates/searchkernel-ir-adapter/src/tests/query_lowering.rs",
            capability["verified_by"],
        )

    def test_every_query_expr_kind_has_one_capability_owner(self) -> None:
        schema = (ROOT / "api" / "searchkernel.fbs").read_text(encoding="utf-8")
        match = re.search(r"union QueryExprKind \{(?P<body>.*?)\n\}", schema, re.DOTALL)
        self.assertIsNotNone(match)
        assert match is not None
        schema_kinds = [
            line.strip().rstrip(",")
            for line in match.group("body").splitlines()
            if line.strip()
        ]

        owners = [
            kind
            for capability in self.ledger["capabilities"]
            if capability["area"] == "query"
            for kind in capability.get("query_expr_kinds", [])
        ]
        self.assertEqual(
            len(owners),
            len(set(owners)),
            "each QueryExprKind must be assigned to only one capability",
        )
        self.assertEqual(
            set(owners),
            set(schema_kinds),
            "the capability ledger must classify every current QueryExprKind",
        )

    def test_unclassified_query_expr_kind_is_rejected_by_validator(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        capability = next(
            capability
            for capability in ledger["capabilities"]
            if capability["id"] == "query.term"
        )
        capability["query_expr_kinds"] = ["TermQuery"]
        with self.assertRaisesRegex(MODULE.LedgerError, "unclassified QueryExprKind"):
            MODULE.validate_ledger(ledger)

    def test_unknown_query_expr_kind_is_rejected(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        capability = next(
            capability
            for capability in ledger["capabilities"]
            if capability["id"] == "query.term"
        )
        capability["query_expr_kinds"].append("QueryAddedWithoutLedgerEntry")
        with self.assertRaisesRegex(MODULE.LedgerError, "unknown QueryExprKind"):
            MODULE.validate_ledger(ledger)

    def test_duplicate_query_expr_kind_is_rejected(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        capability = next(
            capability
            for capability in ledger["capabilities"]
            if capability["id"] == "query.boolean"
        )
        capability["query_expr_kinds"].append("TermQuery")
        with self.assertRaisesRegex(MODULE.LedgerError, "assigned to both"):
            MODULE.validate_ledger(ledger)
    def test_proximity_capability_matches_rust_facade_support(self) -> None:
        capability = next(
            capability
            for capability in self.ledger["capabilities"]
            if capability["id"] == "query.proximity"
        )
        self.assertIn(
            "core/crates/searchkernel/src/query.rs",
            capability["verified_by"],
        )
        self.assertEqual(
            capability["facade_support"]["rust"]["status"],
            "supported",
        )
        self.assertEqual(
            capability["facade_support"]["typescript"]["status"],
            "supported",
        )
        self.assertNotIn(
            "Rust Query",
            " ".join(capability["limitations"]),
        )

    def test_facade_support_rejects_missing_claim_with_matching_symbols(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        capability = next(
            capability
            for capability in ledger["capabilities"]
            if capability["id"] == "query.proximity"
        )
        capability["facade_support"] = {
            "rust": {
                "status": "missing",
                "evidence": [
                    {
                        "path": "core/crates/searchkernel/src/query.rs",
                        "symbols": ["pub fn near", "pub fn onear"],
                    }
                ],
            }
        }
        with self.assertRaisesRegex(MODULE.LedgerError, "claims the facade is missing"):
            MODULE.validate_ledger(ledger)

    def test_facade_support_rejects_new_constructor_for_missing_claim(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            query = root / "clients" / "typescript" / "src" / "ergonomic" / "query.ts"
            query.parent.mkdir(parents=True)
            query.write_text(
                "export const q = {\n"
                "  near(field: string): unknown { return field; },\n"
                "};\n",
                encoding="utf-8",
            )
            capability = next(
                capability
                for capability in ledger["capabilities"]
                if capability["id"] == "query.proximity"
            )
            capability["facade_support"] = {
                "typescript": {
                    "status": "missing",
                    "evidence": [],
                    "absence_evidence": [
                        {
                            "path": "clients/typescript/src/ergonomic/query.ts",
                            "symbols": ["near(", "onear("],
                        }
                    ],
                }
            }
            with self.assertRaisesRegex(
                MODULE.LedgerError, "implementation symbol.*now exist"
            ):
                MODULE._validate_facade_support(
                    capability["facade_support"],
                    "query.proximity.facade_support",
                    root,
                )

    def test_facade_symbol_evidence_ignores_comments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            query = root / "query.ts"
            query.write_text("// near(field: string)\n", encoding="utf-8")
            state = {
                "typescript": {
                    "status": "missing",
                    "evidence": [],
                    "absence_evidence": [
                        {"path": "query.ts", "symbols": ["near("]}
                    ],
                }
            }
            MODULE._validate_facade_support(state, "facade_support", root)

    def test_facade_support_rejects_supported_claim_without_symbols(self) -> None:
        ledger = copy.deepcopy(self.ledger)
        capability = next(
            capability
            for capability in ledger["capabilities"]
            if capability["id"] == "query.proximity"
        )
        capability["facade_support"] = {
            "rust": {
                "status": "supported",
                "evidence": [
                    {
                        "path": "core/crates/searchkernel/src/query.rs",
                        "symbols": ["pub fn removed_proximity_constructor"],
                    }
                ],
            }
        }
        with self.assertRaisesRegex(MODULE.LedgerError, "missing symbol"):
            MODULE.validate_ledger(ledger)


    def test_limitations_include_authoritative_release_scope(self) -> None:
        limitations = MODULE.generated_documents(self.ledger)[ROOT / "LIMITATIONS.md"]
        release_support = MODULE.load_release_support()
        platform = release_support["platform"]
        self.assertIn("## Release scope", limitations)
        self.assertIn(
            f"**{platform['os']} {platform['arch']} {platform['libc']}**",
            limitations,
        )
        for surface in release_support["deferred"]:
            self.assertIn(f"`{surface}`", limitations)

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
