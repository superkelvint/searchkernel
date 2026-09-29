#!/usr/bin/env python3
"""Fail closed when the response conformance field inventory falls behind FBS."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "api/searchkernel.fbs"

# Each field is either emitted in the normalized trace or deliberately
# represented by a named projection. Keep this inventory explicit: a schema
# field addition must be reviewed and classified before the verifier passes.
CLASSIFIED_FIELDS = {
    "Hit": {
        "id": "trace.hit.id",
        "relevance": "trace.hit.relevance",
        "fields": "trace.hit.fields",
        "highlights": "trace.hit.highlights",
        "source": "trace.hit.source",
        "types": "trace.hit.types",
        "match_features": "trace.hit.match_features",
        "summary_features": "trace.hit.summary_features",
        "rank_features": "trace.hit.rank_features",
        "sort_values": "trace.hit.sort_values",
    },
    "ResultGroup": {
        "id": "trace.group.id",
        "label": "trace.group.label",
        "value": "trace.group.value",
        "fields": "trace.group.fields",
        "children": "trace.group.children (ordered)",
        "continuations": "trace.group.continuations",
        "source": "trace.group.source",
        "types": "trace.group.types",
        "relevance": "trace.group.relevance",
        "limits": "trace.group.limits",
        "metadata": "trace.group.metadata",
    },
    "SearchResult": {
        "root": "trace.root",
        "total_hits": "trace.total_hits",
        "coverage": "trace.coverage",
        "timing": "trace.timing",
        "errors": "trace.errors",
        "trace": "trace.trace",
        "metadata": "trace.metadata",
    },
}


def table_fields(schema: str, table: str) -> set[str]:
    """Read top-level field declarations from a FlatBuffers table body."""
    cleaned = re.sub(r"//[^\n]*|/\*.*?\*/", "", schema, flags=re.S)
    match = re.search(rf"\btable\s+{re.escape(table)}\s*\{{", cleaned)
    if match is None:
        raise ValueError(f"schema table is missing: {table}")
    start = match.end()
    depth = 1
    end = start
    while end < len(cleaned) and depth:
        if cleaned[end] == "{":
            depth += 1
        elif cleaned[end] == "}":
            depth -= 1
        end += 1
    if depth:
        raise ValueError(f"schema table is unterminated: {table}")
    body = cleaned[start : end - 1]
    return set(re.findall(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*:", body, re.M))


def verify(schema: str) -> list[str]:
    errors = []
    for table, classified in CLASSIFIED_FIELDS.items():
        actual = table_fields(schema, table)
        expected = set(classified)
        missing = sorted(actual - expected)
        stale = sorted(expected - actual)
        if missing:
            errors.append(f"{table} has unclassified schema fields: {', '.join(missing)}")
        if stale:
            errors.append(f"{table} classifications no longer exist: {', '.join(stale)}")
        for field, reason in classified.items():
            if not reason.strip():
                errors.append(f"{table}.{field} has an empty classification")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", type=Path, default=SCHEMA)
    args = parser.parse_args()
    errors = verify(args.schema.read_text())
    if errors:
        for error in errors:
            print(error)
        return 1
    print("result response schema classification PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
