#!/usr/bin/env python3
"""Fail closed when response schema, corpus coverage, or trace shape drifts."""

from __future__ import annotations

import argparse
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "api/searchkernel.fbs"

# Every field must name a canonical scenario that actually populates it. The
# cross-language runner verifies both fixture coverage and decoded trace shape.
CLASSIFIED_FIELDS = {
    "Hit": {
        "id": "canonical",
        "relevance": "canonical",
        "fields": "canonical",
        "highlights": "canonical",
        "source": "canonical",
        "types": "canonical",
        "match_features": "canonical",
        "summary_features": "canonical",
        "rank_features": "canonical",
        "sort_values": "canonical",
    },
    "ResultGroup": {
        "id": "canonical",
        "label": "canonical",
        "value": "canonical",
        "fields": "canonical",
        "children": "canonical",
        "continuations": "canonical",
        "source": "canonical",
        "types": "canonical",
        "relevance": "canonical",
        "limits": "canonical",
        "metadata": "canonical",
    },
    "SearchResult": {
        "root": "canonical",
        "total_hits": "canonical",
        "coverage": "canonical",
        "timing": "canonical",
        "errors": "canonical",
        "trace": "canonical",
        "metadata": "canonical",
    },
}


def table_fields(schema: str, table: str) -> set[str]:
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
        for field, scenario in classified.items():
            if not scenario.strip():
                errors.append(f"{table}.{field} has no covering scenario")
    return errors


def _result_nodes(payload: dict[str, Any], table: str) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []

    def walk(wrapper: Any) -> None:
        if not isinstance(wrapper, dict):
            return
        kind = wrapper.get("node_type")
        node = wrapper.get("node")
        if kind == table and isinstance(node, dict):
            found.append(node)
        if kind == "ResultGroup" and isinstance(node, dict):
            for child in node.get("children", []):
                walk(child)

    walk(payload.get("root"))
    return found


def verify_fixture_coverage(envelopes: dict[str, dict[str, Any]]) -> list[str]:
    errors = []
    for table, fields in CLASSIFIED_FIELDS.items():
        by_scenario: dict[str, list[str]] = {}
        for field, scenario in fields.items():
            by_scenario.setdefault(scenario, []).append(field)
        for scenario, expected_fields in by_scenario.items():
            envelope = envelopes.get(scenario)
            if envelope is None:
                errors.append(f"{table} coverage scenario is missing: {scenario}")
                continue
            payload = envelope.get("payload")
            if not isinstance(payload, dict):
                errors.append(f"{scenario} has no SearchResult payload")
                continue
            objects = [payload] if table == "SearchResult" else _result_nodes(payload, table)
            if not objects:
                errors.append(f"{scenario} contains no {table} object")
                continue
            for field in expected_fields:
                if not any(field in item for item in objects):
                    errors.append(f"{table}.{field} is not populated by scenario {scenario}")
    return errors


def trace_shape_errors(trace: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(trace, dict):
        return ["SearchResult trace must be an object"]

    expected_root = set(CLASSIFIED_FIELDS["SearchResult"])
    actual_root = set(trace)
    if actual_root != expected_root:
        errors.append(
            "SearchResult trace fields mismatch: "
            f"missing={sorted(expected_root - actual_root)}, extra={sorted(actual_root - expected_root)}"
        )

    def walk(node: Any, path: str) -> None:
        if node is None:
            return
        if not isinstance(node, dict):
            errors.append(f"{path} must be a result-node object")
            return
        kind = node.get("kind")
        if kind == "hit":
            expected = set(CLASSIFIED_FIELDS["Hit"]) | {"kind"}
        elif kind == "group":
            expected = set(CLASSIFIED_FIELDS["ResultGroup"]) | {"kind"}
        else:
            errors.append(f"{path} has unknown kind: {kind!r}")
            return
        actual = set(node)
        if actual != expected:
            errors.append(
                f"{path} fields mismatch: missing={sorted(expected - actual)}, "
                f"extra={sorted(actual - expected)}"
            )
        if kind == "group":
            children = node.get("children")
            if isinstance(children, list):
                for index, child in enumerate(children):
                    walk(child, f"{path}.children[{index}]")
            else:
                errors.append(f"{path}.children must be a list")

    walk(trace.get("root"), "root")
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
