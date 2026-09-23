#!/usr/bin/env python3
"""Tests for executable client capability evidence."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "check-client-contracts.py"
SPEC = importlib.util.spec_from_file_location("check_client_contracts", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CATALOG = {
    "version": 1,
    "surfaces": {
        "rust": {
            "title": "Rust",
            "runtimes": ["native"],
            "evidence_roots": ["rust-tests"],
            "e2e_lanes": [
                {
                    "runtimes": ["native"],
                    "path": "rust-tests/e2e.rs",
                    "command": ["python3", "-c", "pass"],
                }
            ],
        },
        "python": {
            "title": "Python",
            "runtimes": ["grpc"],
            "evidence_roots": ["python-tests"],
        },
    },
    "scenarios": [
        {
            "capability": "result.highlighting",
            "id": "basic",
            "title": "Basic highlights",
        },
        {
            "capability": "result.highlighting",
            "id": "literal-html",
            "title": "Literal HTML",
        },
    ],
}


class ClientContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        (self.root / "docs").mkdir()
        (self.root / "reference" / "client-contracts").mkdir(parents=True)
        (self.root / "rust-tests").mkdir()
        (self.root / "python-tests").mkdir()
        (self.root / "rust-tests" / "e2e.rs").write_text("", encoding="utf-8")
        (self.root / "docs" / "capabilities.json").write_text(
            json.dumps(
                {
                    "version": 1,
                    "capabilities": [
                        {
                            "id": "result.highlighting",
                            "title": "Highlighting",
                            "area": "result",
                        },
                        {
                            "id": "search.pagination",
                            "title": "Pagination",
                            "area": "search",
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )
        (
            self.root / "reference" / "client-contracts" / "catalog.json"
        ).write_text(json.dumps(CATALOG), encoding="utf-8")

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def load(self):
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        catalog = MODULE.load_catalog(
            self.root / "reference" / "client-contracts" / "catalog.json"
        )
        surfaces, scenarios, overrides = MODULE.validate_catalog(
            catalog, global_caps, self.root
        )
        return global_caps, surfaces, scenarios, overrides

    def test_committed_registry_and_reports_are_valid(self) -> None:
        report = MODULE.build_current_report()
        self.assertEqual(
            MODULE.check_documents(MODULE.generated_documents(report)),
            [],
        )

    def test_evidence_is_discovered_and_rendered(self) -> None:
        (self.root / "rust-tests" / "e2e.rs").write_text(
            "// SKC: result.highlighting/basic "
            "kinds=ergonomic,e2e runtime=native\n",
            encoding="utf-8",
        )
        global_caps, surfaces, scenarios, overrides = self.load()
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        self.assertEqual({item["kind"] for item in evidence}, {"ergonomic", "e2e"})
        report = MODULE.build_report(
            global_caps, surfaces, scenarios, evidence, overrides
        )
        markdown = MODULE.render_markdown(report)
        self.assertIn("ergonomic · E2E 1/2 (native)", markdown)
        self.assertIn("rust-tests/e2e.rs", markdown)


    def test_e2e_evidence_requires_ci_lane(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["surfaces"]["rust"].pop("e2e_lanes")
        path.write_text(json.dumps(catalog), encoding="utf-8")
        (self.root / "rust-tests" / "e2e.rs").write_text(
            "// SKC: result.highlighting/basic kinds=e2e runtime=native\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "not covered by an E2E CI lane"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_default_basic_scenario_exists(self) -> None:
        _, _, scenarios, _ = self.load()
        self.assertEqual(scenarios["search.pagination"], {"basic": "Basic behavior"})

    def test_unknown_capability_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: search.nope/basic kinds=e2e runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown capability"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_unknown_scenario_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: result.highlighting/nope kinds=e2e runtime=grpc\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown scenario"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_unknown_runtime_is_rejected(self) -> None:
        (self.root / "python-tests" / "bad.py").write_text(
            "# SKC: result.highlighting/basic kinds=e2e runtime=browser\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "unknown runtime"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_facade_support_rejects_missing_claim_with_ergonomic_evidence(self) -> None:
        (self.root / "python-tests" / "support.py").write_text(
            "# SKC: result.highlighting/basic kinds=ergonomic runtime=grpc\n",
            encoding="utf-8",
        )
        global_caps, surfaces, scenarios, _ = self.load()
        global_caps["result.highlighting"]["facade_support"] = {
            "python": {"status": "missing", "evidence": []}
        }
        evidence = MODULE.collect_evidence(surfaces, scenarios, self.root)
        with self.assertRaisesRegex(MODULE.RegistryError, "claims python is missing"):
            MODULE.validate_facade_support(global_caps, surfaces, evidence)

    def test_malformed_annotation_is_rejected(self) -> None:
        (self.root / "rust-tests" / "bad.rs").write_text(
            "// SKC: result.highlighting/basic e2e native\n",
            encoding="utf-8",
        )
        _, surfaces, scenarios, _ = self.load()
        with self.assertRaisesRegex(MODULE.RegistryError, "malformed SKC annotation"):
            MODULE.collect_evidence(surfaces, scenarios, self.root)

    def test_blocked_override_requires_issue(self) -> None:
        path = self.root / "reference" / "client-contracts" / "catalog.json"
        catalog = json.loads(json.dumps(CATALOG))
        catalog["overrides"] = [
            {
                "capability": "result.highlighting",
                "scenario": "basic",
                "surface": "python",
                "runtime": "grpc",
                "status": "blocked",
                "reason": "waiting on transport",
            }
        ]
        path.write_text(json.dumps(catalog), encoding="utf-8")
        global_caps = MODULE.load_global_capabilities(
            self.root / "docs" / "capabilities.json"
        )
        with self.assertRaisesRegex(MODULE.RegistryError, "needs an issue number"):
            MODULE.validate_catalog(
                MODULE.load_catalog(path), global_caps, self.root
            )


if __name__ == "__main__":
    unittest.main()
