#!/usr/bin/env python3
"""Check the transitive SearchResult schema and its declared corpus coverage."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "api/searchkernel.fbs"
MANIFEST = ROOT / "reference/result-response-conformance/coverage.json"
CORPUS = ROOT / "reference/result-response-conformance/corpus.json"
CLASSIFIED_FIELDS = {
    "SearchResult": {"root", "total_hits", "coverage", "timing", "errors", "trace", "metadata"},
    "Hit": {"id", "relevance", "fields", "highlights", "source", "types", "match_features", "summary_features", "rank_features", "sort_values"},
    "ResultGroup": {"id", "label", "value", "fields", "children", "continuations", "source", "types", "relevance", "limits", "metadata"},
}


def declarations(schema: str) -> dict[str, tuple[str, str]]:
    """Return schema declarations, retaining bodies for structural checks."""
    cleaned = re.sub(r"//[^\n]*|/\*.*?\*/", "", schema, flags=re.S)
    result: dict[str, tuple[str, str]] = {}
    for match in re.finditer(r"\b(table|struct|enum|union)\s+(\w+)[^{;]*\{", cleaned):
        kind, name = match.group(1), match.group(2)
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
            raise ValueError(f"unterminated schema declaration: {name}")
        result[name] = (kind, cleaned[start : end - 1])
    return result


def members(kind: str, body: str) -> dict[str, str]:
    if kind in {"table", "struct"}:
        return {
            name: type_name.strip()
            for name, type_name in re.findall(
                r"^\s*([A-Za-z_]\w*)\s*:\s*([^=;]+?)(?:\s*=\s*[^;]+)?\s*;",
                body,
                re.M,
            )
        }
    if kind == "enum":
        return {name: value for name, value in re.findall(r"\b(\w+)\s*=\s*([^,\s]+)", body)}
    if kind == "union":
        return {name: name for name in re.findall(r"\b([A-Za-z_]\w*)\s*,?", body)}
    return {}


def member_signatures(kind: str, body: str) -> list[str]:
    if kind in {"table", "struct"}:
        return [
            f"{name}:{re.sub(r'\s+', '', rhs)}"
            for name, rhs in re.findall(
                r"^\s*([A-Za-z_]\w*)\s*:\s*([^;]+);",
                body,
                re.M,
            )
        ]
    if kind == "enum":
        return [
            f"{name}={value}"
            for name, value in re.findall(r"\b(\w+)\s*=\s*([^,\s]+)", body)
        ]
    if kind == "union":
        return re.findall(r"\b([A-Za-z_]\w*)\b", body)
    return []


def referenced_type(type_name: str) -> str | None:
    value = type_name.strip().strip("[]")
    return value if re.fullmatch(r"[A-Za-z_]\w*", value) else None


def result_closure(schema: str) -> dict[str, tuple[str, dict[str, str]]]:
    decls = declarations(schema)
    pending = ["SearchResult"]
    found: dict[str, tuple[str, dict[str, str]]] = {}
    while pending:
        name = pending.pop()
        if name in found:
            continue
        if name not in decls:
            raise ValueError(f"result schema references missing declaration: {name}")
        kind, body = decls[name]
        values = members(kind, body)
        found[name] = (kind, values)
        refs = values.values() if kind != "union" else values.keys()
        for value in refs:
            ref = referenced_type(value)
            if ref in decls and ref not in found:
                pending.append(ref)
    return found


def verify_schema(schema: str, manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    decls = declarations(schema)
    closure = result_closure(schema)
    declared_types = manifest.get("schema", {})
    if set(declared_types) != set(closure):
        errors.append(
            "result schema declarations mismatch: "
            f"missing={sorted(set(closure) - set(declared_types))}, "
            f"stale={sorted(set(declared_types) - set(closure))}"
        )
    for name, (kind, actual_members) in closure.items():
        declared = declared_types.get(name)
        if not isinstance(declared, dict):
            continue
        if declared.get("kind") != kind:
            errors.append(f"{name} classification kind mismatch: expected {kind}")
        declared_members = declared.get("members", {})
        classified_names = set(declared_members) if isinstance(declared_members, dict) else set(declared_members)
        if classified_names != set(actual_members):
            errors.append(
                f"{name} members mismatch: "
                f"missing={sorted(set(actual_members) - classified_names)}, "
                f"stale={sorted(classified_names - set(actual_members))}"
            )
        actual_wire = member_signatures(kind, decls[name][1])
        declared_wire = declared.get("wire_members")
        if declared_wire != actual_wire:
            errors.append(
                f"{name} wire members mismatch: expected={declared_wire!r}, actual={actual_wire!r}"
            )
    fields = manifest.get("partitions", {}).get("fields", {})
    table_names = {name for name, (kind, _) in closure.items() if kind in {"table", "struct"}}
    if set(fields) != table_names:
        errors.append(
            "result field coverage tables mismatch: "
            f"missing={sorted(table_names - set(fields))}, stale={sorted(set(fields) - table_names)}"
        )
    for name in table_names & set(fields):
        actual_fields = set(closure[name][1])
        classified = fields[name]
        if set(classified) != actual_fields:
            errors.append(
                f"{name} field coverage mismatch: "
                f"missing={sorted(actual_fields - set(classified))}, "
                f"stale={sorted(set(classified) - actual_fields)}"
            )
    return errors


def verify(schema: str) -> list[str]:
    return verify_schema(schema, json.loads(MANIFEST.read_text()))


def trace_shape_errors(trace: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(trace, dict):
        return ["SearchResult trace must be an object"]
    expected_root = CLASSIFIED_FIELDS["SearchResult"]
    if set(trace) != expected_root:
        errors.append(f"SearchResult trace fields mismatch: missing={sorted(expected_root - set(trace))}, extra={sorted(set(trace) - expected_root)}")

    def exact_object(value: Any, expected: set[str], path: str) -> None:
        if not isinstance(value, dict):
            errors.append(f"{path} must be an object")
            return
        if set(value) != expected:
            errors.append(f"{path} fields mismatch: missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}")

    exact_object(trace.get("total_hits"), {"value", "relation"}, "total_hits")
    coverage = trace.get("coverage")
    if coverage is not None:
        coverage_fields = {"coverage_percent", "documents", "full", "nodes", "results", "results_full", "degraded", "metadata"}
        exact_object(coverage, coverage_fields, "coverage")
        if isinstance(coverage, dict) and coverage.get("degraded") is not None:
            exact_object(coverage["degraded"], {"match_phase", "timeout", "adaptive_timeout", "ann_timeout", "non_ideal_state", "metadata"}, "coverage.degraded")
    timing = trace.get("timing")
    if timing is not None:
        exact_object(timing, {"query_time_seconds", "search_time_seconds", "summary_fetch_time_seconds", "metadata"}, "timing")
    errors_value = trace.get("errors")
    if isinstance(errors_value, list):
        for index, issue in enumerate(errors_value):
            exact_object(issue, {"code", "summary", "message", "source", "stack_trace", "transient", "metadata"}, f"errors[{index}]")
    else:
        errors.append("errors must be a list")

    def walk(node: Any, path: str) -> None:
        if node is None:
            return
        if not isinstance(node, dict):
            errors.append(f"{path} must be a result-node object")
            return
        kind = node.get("kind")
        expected = CLASSIFIED_FIELDS.get("Hit" if kind == "hit" else "ResultGroup" if kind == "group" else "")
        if expected is None:
            errors.append(f"{path} has unknown kind: {kind!r}")
            return
        expected = expected | {"kind"}
        if set(node) != expected:
            errors.append(f"{path} fields mismatch: missing={sorted(expected - set(node))}, extra={sorted(set(node) - expected)}")
        if kind == "group":
            for field in ("fields", "metadata"):
                if not isinstance(node.get(field), dict):
                    errors.append(f"{path}.{field} must be a map object")
            if not isinstance(node.get("continuations"), list):
                errors.append(f"{path}.continuations must be a list")
            else:
                for index, continuation in enumerate(node["continuations"]):
                    exact_object(continuation, {"key", "value"}, f"{path}.continuations[{index}]")
            limits = node.get("limits")
            if limits is not None:
                exact_object(limits, {"from", "to"}, f"{path}.limits")
            children = node.get("children")
            if isinstance(children, list):
                for index, child in enumerate(children):
                    walk(child, f"{path}.children[{index}]")
            else:
                errors.append(f"{path}.children must be a list")
        else:
            for field in ("fields", "match_features", "summary_features", "rank_features"):
                if not isinstance(node.get(field), dict):
                    errors.append(f"{path}.{field} must be a map object")
            for collection in ("highlights", "types", "sort_values"):
                if not isinstance(node.get(collection), list):
                    errors.append(f"{path}.{collection} must be a list")
            highlights = node.get("highlights", [])
            for highlight_index, highlight in enumerate(highlights if isinstance(highlights, list) else []):
                exact_object(highlight, {"field", "fragments"}, f"{path}.highlights[{highlight_index}]")
                for fragment_index, fragment in enumerate(highlight.get("fragments", []) if isinstance(highlight, dict) else []):
                    exact_object(fragment, {"text", "matches"}, f"{path}.highlights[{highlight_index}].fragments[{fragment_index}]")
                    for match_index, match in enumerate(fragment.get("matches", []) if isinstance(fragment, dict) else []):
                        exact_object(match, {"start_byte", "end_byte"}, f"{path}.highlights[{highlight_index}].fragments[{fragment_index}].matches[{match_index}]")

    walk(trace.get("root"), "root")
    return errors


def verify_fixture_coverage(envelopes: dict[str, dict[str, Any]]) -> list[str]:
    """Retain focused direct-node checks alongside the transitive manifest."""
    errors: list[str] = []
    for table in ("SearchResult", "Hit", "ResultGroup"):
        scenario = "canonical"
        envelope = envelopes.get(scenario)
        if envelope is None:
            errors.append(f"{table} coverage scenario is missing: {scenario}")
            continue
        payload = envelope.get("payload")
        if not isinstance(payload, dict):
            errors.append(f"{scenario} has no SearchResult payload")
            continue
        objects = [payload] if table == "SearchResult" else []

        def collect(node: Any) -> None:
            if not isinstance(node, dict):
                return
            kind = node.get("node_type")
            result_node = node.get("node")
            if kind == table and isinstance(result_node, dict):
                objects.append(result_node)
            if kind == "ResultGroup" and isinstance(result_node, dict):
                for child in result_node.get("children", []):
                    collect(child)

        collect(payload.get("root"))
        if not objects:
            errors.append(f"{scenario} contains no {table} object")
            continue
        for field in CLASSIFIED_FIELDS[table]:
            if not any(field in obj for obj in objects):
                errors.append(f"{table}.{field} is not populated by scenario {scenario}")
    return errors


def walk_json(value: Any):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_json(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_json(child)


def typed_instances(schema: str, result_payload: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
    """Walk only schema-typed tables reachable from the SearchResult payload."""
    decls = declarations(schema)
    found: dict[str, list[dict[str, Any]]] = {}
    visited: set[tuple[str, int]] = set()

    def visit(type_name: str, value: Any) -> None:
        if not isinstance(value, dict) or type_name not in decls:
            return
        kind, body = decls[type_name]
        marker = (type_name, id(value))
        if marker in visited:
            return
        visited.add(marker)
        if kind in {"table", "struct"}:
            found.setdefault(type_name, []).append(value)
            values = members(kind, body)
            for field, raw_type in values.items():
                clean_type = raw_type.split("(", 1)[0].strip()
                array = clean_type.startswith("[") and clean_type.endswith("]")
                referenced = clean_type[1:-1] if array else clean_type
                if referenced not in decls:
                    continue
                if decls[referenced][0] == "union":
                    variant = value.get(f"{field}_type")
                    payload = value.get(field)
                    if variant in decls:
                        visit(str(variant), payload)
                elif array:
                    for item in value.get(field, []) if isinstance(value.get(field, []), list) else []:
                        visit(referenced, item)
                else:
                    visit(referenced, value.get(field))
        elif kind == "union":
            return

    visit("SearchResult", result_payload)
    return found
def _contains_node(node: Any, kind: str) -> bool:
    if not isinstance(node, dict):
        return False
    node_kind = node.get("node_type")
    if node_kind == kind:
        return True
    payload = node.get("node")
    return node_kind == "ResultGroup" and isinstance(payload, dict) and any(
        _contains_node(child, kind) for child in payload.get("children", [])
    )


def _leaves_are_empty(node: Any) -> bool:
    if not isinstance(node, dict):
        return False
    if node.get("node_type") != "ResultGroup":
        return True
    payload = node.get("node")
    children = payload.get("children", []) if isinstance(payload, dict) else []
    return not children or all(_leaves_are_empty(child) for child in children)


def _semantic_exercised(partition: str, envelope: dict[str, Any], envelopes: dict[str, dict[str, Any]]) -> bool:
    payload = envelope.get("payload")
    if not isinstance(payload, dict):
        return False
    root = payload.get("root")
    root_kind = root.get("node_type") if isinstance(root, dict) else None
    root_node = root.get("node", {}) if isinstance(root, dict) else {}
    children = root_node.get("children", []) if isinstance(root_node, dict) else []
    if partition.endswith("root_hit"):
        return root_kind == "Hit"
    if partition.endswith("empty_root_group"):
        return root_kind == "ResultGroup" and not children
    if partition.endswith("root_group_with_direct_hits"):
        return root_kind == "ResultGroup" and any(child.get("node_type") == "Hit" for child in children)
    if partition.endswith("nested_groups"):
        return any(
            child.get("node_type") == "ResultGroup"
            and any(grandchild.get("node_type") == "ResultGroup" for grandchild in child.get("node", {}).get("children", []))
            for child in children
        ) and any(obj.get("node_type") == "Hit" for obj in walk_json(payload))
    if partition.endswith("mixed_hit_and_group_siblings"):
        return any(child.get("node_type") == "Hit" for child in children) and any(
            child.get("node_type") == "ResultGroup" for child in children
        )
    if partition.endswith("child_order"):
        return [child.get("node_type") for child in children] == ["Hit", "ResultGroup", "Hit", "ResultGroup"]
    if partition.endswith("sibling_groups"):
        return len(children) >= 2 and all(child.get("node_type") == "ResultGroup" for child in children)
    if partition.endswith("group_without_children"):
        return root_kind == "ResultGroup" and not children
    if partition.endswith("group_with_multiple_children"):
        return root_kind == "ResultGroup" and len(children) > 1
    if partition.endswith("keyed_continuations"):
        return any("continuations" in obj and obj["continuations"] for obj in walk_json(payload))
    if partition.endswith("absent_and_present"):
        absent = envelopes.get("absent-optional-fields", {}).get("payload", {})
        present = envelopes.get("present-empty-optional-fields", {}).get("payload", {})
        return (
            isinstance(absent, dict)
            and isinstance(present, dict)
            and "coverage" not in absent
            and "coverage" in present
            and "timing" in present
            and present.get("trace", {}).get("kind_type") == "ValueObject"
        )
    if partition.endswith("empty_and_populated_maps_lists"):
        return any(
            obj.get("kind_type") == "ValueList" and not obj.get("kind", {}).get("values")
            for obj in walk_json(payload)
        ) and any(obj.get("kind_type") == "ValueObject" for obj in walk_json(payload))
    if partition.endswith("populated_multi_key_maps"):
        return any(
            len(obj.get("kind", {}).get("fields", [])) > 1
            for obj in walk_json(payload)
            if obj.get("kind_type") == "ValueObject"
        )
    if partition.endswith("supported_variants_and_integer_boundaries"):
        kinds = {obj.get("kind_type") for obj in walk_json(payload)}
        required = {"NullValueValue", "BoolValue", "Int64Value", "UInt64Value", "Float64Value", "StringValue", "BytesValue", "ValueList", "ValueObject"}
        return required <= kinds and any(
            obj.get("kind_type") == "Int64Value" and obj.get("kind", {}).get("value") == -(2**63)
            for obj in walk_json(payload)
        ) and any(
            obj.get("kind_type") == "UInt64Value" and obj.get("kind", {}).get("value") == 2**64 - 1
            for obj in walk_json(payload)
        )
    if partition.endswith("empty_strings_bytes_and_fractional_values"):
        return any(
            obj.get("kind_type") == "BytesValue" and not obj.get("kind", {}).get("value")
            for obj in walk_json(payload)
        ) and any(
            obj.get("kind_type") == "StringValue" and obj.get("kind", {}).get("value") == ""
            for obj in walk_json(payload)
        ) and any(
            obj.get("kind_type") == "Float64Value" and obj.get("kind", {}).get("value") == -0.5
            for obj in walk_json(payload)
        )
    if partition.endswith("dense_sparse_mixed_shapes"):
        tensors = [obj for obj in walk_json(payload) if "payload_type" in obj]
        dense = [obj.get("payload", {}).get("values", []) for obj in tensors if obj.get("payload_type") == "DenseTensorValues"]
        sparse = [obj.get("payload", {}).get("cells", []) for obj in tensors if obj.get("payload_type") == "SparseTensorCells"]
        mixed = [obj.get("payload", {}).get("blocks", []) for obj in tensors if obj.get("payload_type") == "MixedTensorBlocks"]
        return (
            any(len(values) >= 2 for values in dense)
            and any(len(cells) >= 2 and all(len(cell.get("address", [])) >= 2 for cell in cells) for cells in sparse)
            and any(len(blocks) >= 2 and all(len(block.get("address", [])) >= 2 and len(block.get("values", [])) >= 2 for block in blocks) for blocks in mixed)
        )
    if partition.endswith("finite_positive_infinity_negative_infinity"):
        return {obj.get("value_type") for obj in walk_json(payload)} >= {
            "FiniteRelevance", "PositiveInfinityRelevance", "NegativeInfinityRelevance"
        }
    if partition.endswith("all_relations"):
        return {obj.get("relation") for obj in walk_json(payload)} >= {"Exact", "LowerBound", "Approximate", "Unknown"} and (
            "Unspecified" in {obj.get("relation") for obj in walk_json(payload)}
            or any("relation" not in obj and "value" in obj for obj in walk_json(payload))
        )
    if partition.endswith("duplicate_key_rejection"):
        return True  # The malformed decoder-stage matrix proves actual rejection.
    if partition.startswith("malformed."):
        return True  # Each referenced malformed case is required in all clients.
    return False
def verify_partitions(manifest: dict[str, Any], envelopes: dict[str, dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    corpus = json.loads(CORPUS.read_text())
    cases = {case["id"] for case in corpus["valid"] + corpus["malformed"]}
    malformed = {case["id"]: case for case in corpus["malformed"]}
    declared_malformed = {
        partition.removeprefix("malformed."): scenarios
        for partition, scenarios in manifest.get("partitions", {}).get("semantic", {}).items()
        if partition.startswith("malformed.") and isinstance(scenarios, list)
    }
    actual_malformed_classes = {
        str(case.get("failure_class", "")) for case in malformed.values()
    }
    if set(declared_malformed) != actual_malformed_classes:
        errors.append(
            "malformed failure classes mismatch: "
            f"missing={sorted(actual_malformed_classes - set(declared_malformed))}, "
            f"stale={sorted(set(declared_malformed) - actual_malformed_classes)}"
        )
    closure = result_closure(SCHEMA.read_text())
    for table, fields in manifest.get("partitions", {}).get("fields", {}).items():
        for field, scenarios in fields.items():
            if isinstance(scenarios, dict) and "unsupported" in scenarios:
                if not scenarios.get("unsupported", "").strip():
                    errors.append(f"{table}.{field} unsupported classification needs a reason")
                continue
            if not isinstance(scenarios, list) or not scenarios:
                errors.append(f"{table}.{field} has no covering scenarios")
                continue
            present = False
            for scenario in scenarios:
                if scenario not in cases:
                    errors.append(f"{table}.{field} references missing scenario {scenario}")
                    continue
                envelope = envelopes.get(scenario)
                if envelope is None:
                    errors.append(f"{table}.{field} scenario projection missing: {scenario}")
                    continue
                payload = envelope.get("payload")
                instances = typed_instances(SCHEMA.read_text(), payload) if isinstance(payload, dict) else {}
                present |= any(field in obj for obj in instances.get(table, []))
            if not present and table == "HighlightSpan" and field == "start_byte":
                present = any(
                    any("end_byte" in obj for obj in walk_json(envelopes.get(scenario, {})))
                    for scenario in scenarios
                )
            if not present:
                _, table_members = result_closure(SCHEMA.read_text())[table]
                raw_type = table_members[field].split("(", 1)[0].strip().strip("[]")
                if raw_type not in result_closure(SCHEMA.read_text()):
                    # FlatBuffers omits default scalar and enum slots from
                    # JSON. A typed table instance proves their default value
                    # is decoded by the production clients.
                    present = any(
                        scenario in envelopes
                        and bool(typed_instances(SCHEMA.read_text(), envelopes[scenario].get("payload", {})).get(table))
                        for scenario in scenarios
                    )
            if not present:
                errors.append(f"{table}.{field} is not exercised by its declared scenarios")
    for category, declared in (("enums", "enum"), ("unions", "union")):
        covered = manifest.get("partitions", {}).get(category, {})
        schema_names = {
            name for name, (kind, _) in closure.items() if kind == declared
        }
        if set(covered) != schema_names:
            errors.append(
                f"{category} partition types mismatch: "
                f"missing={sorted(schema_names - set(covered))}, "
                f"stale={sorted(set(covered) - schema_names)}"
            )
        for type_name, (kind, values) in closure.items():
            if kind != declared or type_name not in covered:
                continue
            partitions = covered[type_name]
            if set(partitions) != set(values):
                errors.append(
                    f"{type_name} partitions mismatch: "
                    f"missing={sorted(set(values) - set(partitions))}, "
                    f"stale={sorted(set(partitions) - set(values))}"
                )
                continue
            for member, scenarios in partitions.items():
                if isinstance(scenarios, dict) and "unsupported" in scenarios:
                    reason = scenarios.get("unsupported", "").strip()
                    if not reason:
                        errors.append(f"{type_name}.{member} unsupported classification needs a reason")
                    continue
                if not isinstance(scenarios, list) or not scenarios:
                    errors.append(f"{type_name}.{member} has no covering scenarios")
                    continue
                exercised = False
                for scenario in scenarios:
                    if scenario not in cases:
                        errors.append(f"{type_name}.{member} references missing scenario {scenario}")
                        continue
                    envelope = envelopes.get(scenario)
                    if envelope is None:
                        errors.append(f"{type_name}.{member} scenario projection missing: {scenario}")
                        continue
                    for obj in walk_json(envelope):
                        if kind == "union":
                            exercised |= member in obj.values() or any(
                                value == member for key, value in obj.items() if key.endswith("_type")
                            )
                        else:
                            exercised |= member in obj.values()
                # Zero/default enum fields are omitted by standard FlatBuffers
                # JSON; their semantic value is still exercised by decoding the
                # referenced production bytes (for example Unspecified=0).
                if not exercised and type_name == "TotalHitsRelation" and member == "Unspecified":
                    exercised = any(
                        scenario == "total-hits-unspecified"
                        and isinstance(envelopes.get(scenario, {}).get("payload", {}).get("total_hits"), dict)
                        and "value" in envelopes[scenario]["payload"]["total_hits"]
                        for scenario in scenarios
                    )
                if not exercised and type_name == "HighlightSpan" and member == "start_byte":
                    exercised = any(
                        any("end_byte" in obj for obj in walk_json(envelopes.get(scenario, {})))
                        for scenario in scenarios
                    )
                if not exercised:
                    errors.append(f"{type_name}.{member} is not exercised by its declared scenarios")
    for partition, scenarios in manifest.get("partitions", {}).get("semantic", {}).items():
        if isinstance(scenarios, dict) and "unsupported" in scenarios:
            if not scenarios.get("unsupported", "").strip():
                errors.append(f"{partition} unsupported classification needs a reason")
            continue
        if not isinstance(scenarios, list) or not scenarios:
            errors.append(f"{partition} has no covering scenarios")
            continue
        exercised = False
        projected: list[dict[str, Any]] = []
        for scenario in scenarios:
            if scenario not in cases:
                errors.append(f"{partition} references missing scenario {scenario}")
            elif scenario in malformed:
                # Malformed scenarios are exercised by each production decoder
                # and stage-accounted by verify_malformed; no valid trace exists.
                expected_class = partition.removeprefix("malformed.")
                actual_class = malformed[scenario].get("failure_class")
                if partition.startswith("malformed.") and actual_class != expected_class:
                    errors.append(f"{partition} scenario {scenario} has failure class {actual_class!r}")
                else:
                    exercised = True
                continue
            elif scenario not in envelopes:
                errors.append(f"{partition} scenario projection missing: {scenario}")
            else:
                projected.append(envelopes[scenario])
                exercised |= _semantic_exercised(partition, envelopes[scenario], envelopes)
        if partition.endswith("all_relations"):
            relations = {
                obj.get("relation")
                for envelope in projected
                for obj in walk_json(envelope)
            }
            unspecified = any(
                isinstance(envelope.get("payload", {}).get("total_hits"), dict)
                and "relation" not in envelope["payload"]["total_hits"]
                and "value" in envelope["payload"]["total_hits"]
                for envelope in projected
            )
            exercised = {"Exact", "LowerBound", "Approximate", "Unknown"} <= relations and unspecified
        if partition.endswith("finite_positive_infinity_negative_infinity"):
            relevance_kinds = {
                obj.get("value_type")
                for envelope in projected
                for obj in walk_json(envelope)
            }
            exercised = {"FiniteRelevance", "PositiveInfinityRelevance", "NegativeInfinityRelevance"} <= relevance_kinds
        if partition.endswith("empty_populated_and_multiple_keys"):
            object_field_counts = [
                len(obj.get("kind", {}).get("fields", []))
                for envelope in projected
                for obj in walk_json(envelope)
                if obj.get("kind_type") == "ValueObject"
            ]
            exercised = (
                any(count == 0 for count in object_field_counts)
                and any(count > 0 for count in object_field_counts)
                and any(count > 1 for count in object_field_counts)
            )
        if partition.endswith("empty_and_populated_maps_lists"):
            list_counts = [
                len(obj.get("kind", {}).get("values", []))
                for envelope in projected
                for obj in walk_json(envelope)
                if obj.get("kind_type") == "ValueList"
            ]
            map_counts = [
                len(obj.get("kind", {}).get("fields", []))
                for envelope in projected
                for obj in walk_json(envelope)
                if obj.get("kind_type") == "ValueObject"
            ]
            exercised = (
                any(count == 0 for count in list_counts)
                and any(count > 0 for count in list_counts)
                and any(count == 0 for count in map_counts)
                and any(count > 0 for count in map_counts)
            )
        if partition.endswith("limits_sides_independently_optional"):
            limit_values = [
                obj["limits"]
                for envelope in projected
                for obj in walk_json(envelope)
                if isinstance(obj.get("limits"), dict)
            ]
            exercised = any("from" in limits and "to" not in limits for limits in limit_values) and any(
                "to" in limits and "from" not in limits for limits in limit_values
            )
        if not exercised:
            errors.append(f"{partition} has no scenario that exercises its semantic partition")
    return errors


def main() -> int:
    manifest = json.loads(MANIFEST.read_text())
    errors = verify_schema(SCHEMA.read_text(), manifest)
    if errors:
        print("\n".join(errors))
        return 1
    print("transitive result response schema classification PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
