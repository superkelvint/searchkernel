#!/usr/bin/env python3
"""Check that portable SearchRequest semantics have one public entry point.

This is intentionally a structural check, not a source snapshot. It looks for
public functions that accept a generated SearchRequest component and return
resolved IR. Existing compatibility methods are permitted only while their
bodies still delegate through the canonical decoder/resolver path.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = ROOT / "core" / "crates" / "searchkernel-ir" / "src" / "api_to_ir"

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

# Guard the architectural property, not the complete function text. Each
# compatibility surface must retain the call(s) that route its component
# through the canonical structural decoder/resolver used by full search.
DELEGATION_REQUIREMENTS = {
    ("search.rs", "query_to_ir"): ("self.typed_query_to_ir(",),
    ("search.rs", "search_fields_to_ir"): ("resolve_search_fields_at(",),
    ("search.rs", "sort_to_ir"): ("self.sort_to_input(", "resolve_sort("),
    ("search.rs", "projection_to_ir"): ("resolve_projection(",),
    ("search.rs", "highlighting_to_ir"): ("resolve_highlighting(",),
    ("ranking.rs", "ranking_request_to_ir"): ("ranking_to_input(", "resolve_search("),
    ("filter.rs", "filter_to_ir"): ("self.filter_to_ir_at(",),
    ("filter.rs", "comparison_to_ir"): ("self.comparison_to_ir_at(",),
    ("filter.rs", "range_to_ir"): ("self.range_to_ir_at(",),
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


def function_parts(source: str, start: int) -> tuple[str, str]:
    """Return a public function signature and body.

    The body scanner ignores braces inside quoted strings and comments so
    format strings cannot accidentally truncate the structural check.
    """

    opening = source.find("{", start)
    if opening == -1:
        return source[start:], ""

    depth = 0
    index = opening
    in_string = False
    escaped = False
    line_comment = False
    block_comment_depth = 0

    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""

        if line_comment:
            if char == "\n":
                line_comment = False
            index += 1
            continue

        if block_comment_depth:
            if char == "/" and next_char == "*":
                block_comment_depth += 1
                index += 2
                continue
            if char == "*" and next_char == "/":
                block_comment_depth -= 1
                index += 2
                continue
            index += 1
            continue

        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue

        if char == "/" and next_char == "/":
            line_comment = True
            index += 2
            continue
        if char == "/" and next_char == "*":
            block_comment_depth = 1
            index += 2
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start:opening], source[opening + 1:index]
        index += 1

    return source[start:opening], source[opening + 1:]


def component_types_in(signature: str) -> list[str]:
    return [name for name in COMPONENT_TYPES if re.search(rf"\b{name}\b", signature)]


def inspect_function(path: Path, name: str, signature: str, body: str) -> str | None:
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
        required = DELEGATION_REQUIREMENTS[(path.name, name)]
        missing = [marker for marker in required if marker not in body]
        if not missing:
            return None
        return (
            f"{path.relative_to(ROOT)}: compatibility method {name} no longer "
            f"delegates through the canonical path; missing {', '.join(missing)}"
        )

    return (
        f"{path.relative_to(ROOT)}: public function {name} accepts portable "
        f"SearchRequest component(s) {', '.join(components)} and returns IR"
    )


def violations_in(path: Path) -> list[str]:
    source = path.read_text(encoding="utf-8")
    violations: list[str] = []

    for match in FUNCTION_RE.finditer(source):
        signature, body = function_parts(source, match.start())
        violation = inspect_function(path, match.group(1), signature, body)
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
