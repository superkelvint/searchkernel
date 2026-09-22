#!/usr/bin/env python3
"""Regression tests for the SearchRequest semantic-entry-point guard."""

from __future__ import annotations

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "tools" / "check-search-semantic-entry-points.py"


def run_checker(source: str) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as directory:
        fixture_root = Path(directory)
        checker = fixture_root / "tools" / CHECKER.name
        checker.parent.mkdir()
        shutil.copy2(CHECKER, checker)
        checker.chmod(0o755)

        source_root = (
            fixture_root
            / "core"
            / "crates"
            / "searchkernel-ir"
            / "src"
            / "api_to_ir"
        )
        source_root.mkdir(parents=True)
        (source_root / "search.rs").write_text(source, encoding="utf-8")

        return subprocess.run(
            [str(checker)],
            cwd=fixture_root,
            text=True,
            capture_output=True,
            check=False,
        )


class SearchSemanticEntryPointCheckerTests(unittest.TestCase):
    def test_operation_decoder_and_shared_resolver_are_accepted(self) -> None:
        result = run_checker(
            """
            impl IndexContext {
                pub fn search_to_ir(
                    &self,
                    request: api::SearchRequest<'_>,
                ) -> Result<SearchRequestIr> {
                    resolve_search(self, decode_search_input(self, request)?)
                }

                pub(crate) fn query_source_to_input(
                    &self,
                    source: Option<api::QuerySource<'_>>,
                ) -> Result<QuerySourceInput> {
                    decode_query_source(source)
                }
            }

            pub fn resolve_query(
                context: &IndexContext,
                input: QueryInput,
            ) -> Result<QueryIr> {
                resolve_query_input(context, input)
            }
            """
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_existing_component_delegate_is_accepted(self) -> None:
        result = run_checker(
            """
            impl IndexContext {
                pub fn sort_to_ir(
                    &self,
                    sort: api::SortExpr<'_>,
                ) -> Result<SortIr> {
                    let input = self.sort_to_input(sort, "sort")?;
                    resolve_sort(self, &input, "sort")
                }
            }
            """
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_existing_component_name_with_independent_body_is_rejected(self) -> None:
        result = run_checker(
            """
            impl IndexContext {
                pub fn sort_to_ir(
                    &self,
                    sort: api::SortExpr<'_>,
                ) -> Result<SortIr> {
                    independently_normalize(sort)
                }
            }
            """
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sort_to_ir", result.stdout + result.stderr)
        self.assertIn("no longer delegates", result.stdout + result.stderr)

    def test_new_public_component_ir_entry_point_is_rejected(self) -> None:
        result = run_checker(
            """
            impl IndexContext {
                pub fn new_query_to_ir(
                    &self,
                    expression: api::QueryExpr<'_>,
                ) -> Result<QueryIr> {
                    independently_normalize(expression)
                }
            }
            """
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new_query_to_ir", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
