#!/usr/bin/env python3
"""Regression tests for the IR boundary constructor verifier."""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check-ir-boundary.sh"


def run_checker(
    model_source: str, production_source: str | None = None
) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as directory:
        fixture_root = Path(directory)
        checker = fixture_root / "tools" / "check-ir-boundary.sh"
        checker.parent.mkdir()
        shutil.copy2(CHECKER, checker)
        checker.chmod(0o755)

        model = fixture_root / "core" / "crates" / "searchkernel-ir" / "src" / "model"
        model.mkdir(parents=True)
        (model / "fixture.rs").write_text(model_source, encoding="utf-8")
        (model / "query.rs").write_text(model_source, encoding="utf-8")
        if production_source is not None:
            source = (
                fixture_root
                / "core"
                / "crates"
                / "searchkernel-ir"
                / "src"
                / "fixture.rs"
            )
            source.write_text(production_source, encoding="utf-8")
        core = model.parent
        (core / "backend.rs").touch()
        (core / "error.rs").touch()

        return subprocess.run(
            [str(checker)],
            cwd=fixture_root,
            text=True,
            capture_output=True,
            check=False,
        )


class IrBoundaryCheckerTests(unittest.TestCase):
    def test_repository_test_feature_gates_are_accepted(self) -> None:
        result = run_checker(
            """
            pub struct DocumentIr;

            impl DocumentIr {
                #[cfg(any(test, feature = "test"))]
                pub fn new() {}
            }
            """
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_ungated_aggregate_constructor_is_rejected(self) -> None:
        result = run_checker(
            """
            pub struct DocumentIr;

            impl DocumentIr {
                pub fn new() {}
            }
            """
        )
        self.assertNotEqual(result.returncode, 0)

    def test_legacy_query_variants_are_rejected(self) -> None:
        result = run_checker(
            """pub enum QueryIr {
                Term,
                Typed,
            }
            """
        )
        self.assertNotEqual(result.returncode, 0)

    def test_legacy_query_construction_in_production_is_rejected(self) -> None:
        result = run_checker(
            "pub enum QueryIr { Typed }\n",
            "fn build() { let _query = QueryIr::Term; }\n",
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
