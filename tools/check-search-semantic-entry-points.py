#!/usr/bin/env python3
"""Check that portable SearchRequest semantics have one public entry point.

This is intentionally a structural check, not a source snapshot.  It looks
for public functions that accept a generated SearchRequest component and
return resolved IR.  The existing component methods are compatibility
delegates retained by the API; a newly named method must be reviewed instead
of silently becoming another semantic entry point.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = ROOT / "core" / "crates" / "searchkernel-ir" / "src" / "api_to_ir"

# These are the only portable SearchRequest component methods currently
# exposed by IndexContext.  They are compatibility surfaces whose bodies must
# continue to delegate to the shared input/resolver path.  This name-and-file
# allowlist is deliberately narrower than a full-file snapshot: a new public
# component method, regardless of its implementation or file, is reported.
KNOWN_COMPONENT_METHODS = {
    "search.rs": frozenset(
        {
            "query_to_ir",
            "search_fields_to_ir",
            "sort_to_ir",
            "projection_to_ir",
            "highlighting_to_ir",
        }
    ),
    "ranking.rs": frozenset({"ranking_request_to_ir"}),
    "filter.rs": frozenset({"filter_to_ir", "comparison_to_ir", "range_to_ir"}),
}

OPERATION_METHODS = {"search.rs": frozenset({"search_to_ir", "inspect_query"})}

COMPONENT_TYPES = (
    "SearchRequest",
    "QueryExpr",
    "QuerySource",
    "SearchFieldSelection",
    "SortExpr",
    "FieldProjection",
    "HighlightRequest",
    "RankingRequest",
    "FilterExpr",
    "FilterComparison",
    "FilterRange",
)

FUNCTION_RE = re.compile(
    r"(?m)^\s*pub\s+(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
)


def function_header(source: str, start: int) -> str:
    """Return a public function signature through its opening body brace."""

    opening = source.find("{", start)
    if opening == -1:
        return source[start:]
    return source[start:opening]


def component_types_in(signature: str) -> list[str]:
    return [name for name in COMPONENT_TYPES if re.search(rf"\b{name}\b", signature)]


def inspect_function(path: Path, name: str, signature: str) -> str | None:
    components = component_types_in(signature)
    if not components or "Ir" not in signature:
        return None
    if (
        path.name in OPERATION_METHODS
        and name in OPERATION_METHODS[path.name]
        and "&self" in signature
    ):
        return None
    if (
        path.name in KNOWN_COMPONENT_METHODS
        and name in KNOWN_COMPONENT_METHODS[path.name]
        and "&self" in signature
    ):
        return None
    return (
        f"{path.relative_to(ROOT)}: public function {name} accepts portable "
        f"SearchRequest component(s) {', '.join(components)} and returns IR"
    )


def violations_in(path: Path) -> list[str]:
    source = path.read_text(encoding="utf-8")
    violations: list[str] = []

    # This intentionally inspects public free functions and IndexContext
    # methods alike.  Private helpers are shared semantic implementation, not
    # externally visible entry points, and are outside this guard's scope.
    for match in FUNCTION_RE.finditer(source):
        violation = inspect_function(path, match.group(1), function_header(source, match.start()))
        if violation:
            violations.append(violation)
    return violations


def main() -> int:
    violations = [
        violation
        for path in sorted(SOURCE_ROOT.glob("*.rs"))
        if not path.name.endswith("tests.rs")
        for violation in violations_in(path)
    ]
    if violations:
        print("SearchRequest semantic-entry-point guard FAILED", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1
    print("SearchRequest semantic-entry-point guard PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
