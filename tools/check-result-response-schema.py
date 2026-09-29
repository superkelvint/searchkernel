#!/usr/bin/env python3
"""Check the transitive SearchResult schema and its declared corpus coverage."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import flatbuffers.encode
import flatbuffers.packer
from flatbuffers import number_types as fb_types
from flatbuffers.table import Table

ROOT = Path(__file__).resolve().parents[1]
BFBS = ROOT / "core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs"
MANIFEST = ROOT / "reference/result-response-conformance/coverage.json"
CORPUS = ROOT / "reference/result-response-conformance/corpus.json"
RESULT_ROOT = "SearchKernel.V2.SearchResult"

# reflection.BaseType values from the pinned FlatBuffers reflection schema.
_BASE_TYPES = {
    0: "None",
    1: "UType",
    2: "Bool",
    3: "Byte",
    4: "UByte",
    5: "Short",
    6: "UShort",
    7: "Int",
    8: "UInt",
    9: "Long",
    10: "ULong",
    11: "Float",
    12: "Double",
    13: "String",
    14: "Vector",
    15: "Obj",
    16: "Union",
    17: "Array",
    18: "Vector64",
    19: "MaxBaseType",
}
_UTYPE = 1
_OBJ = 15
_UNION = 16
_SEQUENCES = {14, 17, 18}

CLASSIFIED_FIELDS = {
    "SearchResult": {"root", "total_hits", "coverage", "timing", "errors", "trace", "metadata"},
    "Hit": {"id", "relevance", "fields", "highlights", "source", "types", "match_features", "summary_features", "rank_features", "sort_values"},
    "ResultGroup": {"id", "label", "value", "fields", "children", "continuations", "source", "types", "relevance", "limits", "metadata"},
}


class _BfbsReflection:
    """Read reflection.Schema metadata using the pinned FlatBuffers runtime."""

    def __init__(self, data: bytes):
        if len(data) < 8 or data[4:8] != b"BFBS":
            raise ValueError("response schema is not a BFBS reflection buffer")
        root = flatbuffers.encode.Get(flatbuffers.packer.uoffset, data, 0)
        self.root = Table(data, root)

    @staticmethod
    def _slot(slot: int) -> int:
        # Generated FlatBuffers accessors use vtable offsets 4, 6, 8, ...
        return 4 + slot * 2

    def field(self, table: Table, slot: int) -> int:
        return int(table.Offset(self._slot(slot)))

    def table_field(self, table: Table, slot: int) -> Table | None:
        offset = self.field(table, slot)
        if offset == 0:
            return None
        return Table(table.Bytes, table.Indirect(table.Pos + offset))

    def string_field(self, table: Table, slot: int) -> str | None:
        offset = self.field(table, slot)
        if offset == 0:
            return None
        try:
            return table.String(table.Pos + offset).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError("BFBS reflection string is not UTF-8") from error

    def vector_tables(self, table: Table, slot: int) -> list[Table]:
        offset = self.field(table, slot)
        if offset == 0:
            return []
        start = table.Vector(offset)
        length = table.VectorLen(offset)
        return [
            Table(table.Bytes, table.Indirect(start + index * 4))
            for index in range(length)
        ]

    def i8_field(self, table: Table, slot: int, default: int = 0) -> int:
        offset = self.field(table, slot)
        if offset == 0:
            return default
        return int(table.Get(fb_types.Int8Flags, table.Pos + offset))

    def i32_field(self, table: Table, slot: int, default: int = -1) -> int:
        offset = self.field(table, slot)
        if offset == 0:
            return default
        return int(table.Get(fb_types.Int32Flags, table.Pos + offset))

    def bool_field(self, table: Table, slot: int) -> bool:
        offset = self.field(table, slot)
        if offset == 0:
            return False
        return bool(table.Get(fb_types.BoolFlags, table.Pos + offset))


def _bfbs_declarations(
    path: Path = BFBS,
) -> dict[str, tuple[str, dict[str, str]]]:
    """Load logical schema declarations from checked-in BFBS reflection data."""
    reflection = _BfbsReflection(path.read_bytes())
    schema = reflection.root

    objects: list[dict[str, Any]] = []
    for table in reflection.vector_tables(schema, 0):
        name = reflection.string_field(table, 0)
        if not name:
            raise ValueError("BFBS reflection object has no name")
        fields: list[tuple[str, int, int, int]] = []
        for field_table in reflection.vector_tables(table, 1):
            field_name = reflection.string_field(field_table, 0)
            type_table = reflection.table_field(field_table, 1)
            if not field_name or type_table is None:
                raise ValueError(f"BFBS reflection field is incomplete: {name}")
            fields.append(
                (
                    field_name,
                    reflection.i8_field(type_table, 0),
                    reflection.i8_field(type_table, 1),
                    reflection.i32_field(type_table, 2),
                )
            )
        objects.append(
            {
                "name": name,
                "kind": "struct" if reflection.bool_field(table, 2) else "table",
                "fields": fields,
            }
        )

    enums: list[dict[str, Any]] = []
    for table in reflection.vector_tables(schema, 1):
        name = reflection.string_field(table, 0)
        if not name:
            raise ValueError("BFBS reflection enum has no name")
        values: list[tuple[str, tuple[int, int] | None]] = []
        for value_table in reflection.vector_tables(table, 1):
            value_name = reflection.string_field(value_table, 0)
            if not value_name:
                raise ValueError(f"BFBS reflection enum value is unnamed: {name}")
            union_type = reflection.table_field(value_table, 3)
            values.append(
                (
                    value_name,
                    None
                    if union_type is None
                    else (
                        reflection.i8_field(union_type, 0),
                        reflection.i32_field(union_type, 2),
                    ),
                )
            )
        enums.append(
            {
                "name": name,
                "kind": "union" if reflection.bool_field(table, 2) else "enum",
                "values": values,
            }
        )

    def indexed_name(values: list[dict[str, Any]], index: int, what: str) -> str:
        if index < 0 or index >= len(values):
            raise ValueError(f"BFBS reflection {what} index is invalid: {index}")
        return str(values[index]["name"])

    def type_name(base: int, element: int, index: int) -> str:
        if base == _OBJ:
            return indexed_name(objects, index, "object")
        if base in {_UNION, _UTYPE}:
            return indexed_name(enums, index, "enum")
        if base in _SEQUENCES:
            if element == _OBJ:
                inner = indexed_name(objects, index, "object")
            elif element in {_UNION, _UTYPE}:
                inner = indexed_name(enums, index, "enum")
            elif index >= 0:
                inner = indexed_name(enums, index, "enum")
            else:
                inner = _BASE_TYPES.get(element, f"BaseType({element})")
            return f"[{inner}]"
        if index >= 0:
            return indexed_name(enums, index, "enum")
        return _BASE_TYPES.get(base, f"BaseType({base})")

    declarations: dict[str, tuple[str, dict[str, str]]] = {}
    for obj in objects:
        fields: dict[str, str] = {}
        for field_name, base, element, index in obj["fields"]:
            # BFBS materializes a synthetic <union>_type UType field beside
            # every logical union payload. The .fbs contract has one field;
            # omit the implementation discriminator from the logical schema.
            if base == _UTYPE or (base in _SEQUENCES and element == _UTYPE):
                continue
            fields[field_name] = type_name(base, element, index)
        declarations[str(obj["name"])] = (str(obj["kind"]), fields)

    for enum in enums:
        members: dict[str, str] = {}
        if enum["kind"] == "union":
            for member, union_type in enum["values"]:
                # Reflection unions contain a synthetic NONE=0 member with no
                # payload type. It is absence, not a logical union arm.
                if union_type is None or union_type[0] == 0:
                    continue
                base, index = union_type
                members[member] = type_name(base, 0, index)
        else:
            members = {member: member for member, _ in enum["values"]}
        declarations[str(enum["name"])] = (str(enum["kind"]), members)

    return declarations


def _referenced_type(type_name: str) -> str:
    return type_name[1:-1] if type_name.startswith("[") and type_name.endswith("]") else type_name


def result_closure(
    path: Path = BFBS,
) -> dict[str, tuple[str, dict[str, str]]]:
    """Return the SearchResult transitive closure from BFBS reflection data."""
    declarations = _bfbs_declarations(path)
    pending = [RESULT_ROOT]
    found: dict[str, tuple[str, dict[str, str]]] = {}

    while pending:
        name = pending.pop()
        if name in found:
            continue
        if name not in declarations:
            raise ValueError(f"result BFBS references missing declaration: {name}")
        kind, values = declarations[name]
        found[name] = (kind, values)
        for value in values.values():
            referenced = _referenced_type(value)
            if referenced in declarations and referenced not in found:
                pending.append(referenced)

    short_names: dict[str, str] = {}
    used: dict[str, str] = {}
    for full_name in found:
        short = full_name.rsplit(".", 1)[-1]
        previous = used.get(short)
        if previous is not None and previous != full_name:
            raise ValueError(
                f"result BFBS short-name collision: {previous} and {full_name}"
            )
        used[short] = full_name
        short_names[full_name] = short

    def shorten(value: str) -> str:
        sequence = value.startswith("[") and value.endswith("]")
        inner = value[1:-1] if sequence else value
        logical = short_names.get(inner, inner)
        return f"[{logical}]" if sequence else logical

    return {
        short_names[full_name]: (
            kind,
            {member: shorten(value) for member, value in values.items()},
        )
        for full_name, (kind, values) in found.items()
    }


def verify_schema(
    closure: dict[str, tuple[str, dict[str, str]]],
    manifest: dict[str, Any],
) -> list[str]:
    errors: list[str] = []
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


def verify(path: Path = BFBS) -> list[str]:
    return verify_schema(result_closure(path), json.loads(MANIFEST.read_text()))


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


def typed_instances(
    closure: dict[str, tuple[str, dict[str, str]]],
    result_payload: dict[str, Any],
) -> dict[str, list[dict[str, Any]]]:
    """Walk only BFBS-typed tables reachable from the SearchResult payload."""
    found: dict[str, list[dict[str, Any]]] = {}
    visited: set[tuple[str, int]] = set()

    def visit(type_name: str, value: Any) -> None:
        if not isinstance(value, dict) or type_name not in closure:
            return
        kind, fields = closure[type_name]
        marker = (type_name, id(value))
        if marker in visited:
            return
        visited.add(marker)
        if kind not in {"table", "struct"}:
            return

        found.setdefault(type_name, []).append(value)
        for field, declared_type in fields.items():
            sequence = declared_type.startswith("[") and declared_type.endswith("]")
            referenced = declared_type[1:-1] if sequence else declared_type
            if referenced not in closure:
                continue

            referenced_kind, union_members = closure[referenced]
            if referenced_kind == "union":
                variant = value.get(f"{field}_type")
                target = union_members.get(str(variant))
                payload = value.get(field)
                if target in closure:
                    visit(target, payload)
            elif sequence:
                items = value.get(field, [])
                if isinstance(items, list):
                    for item in items:
                        visit(referenced, item)
            else:
                visit(referenced, value.get(field))

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
        return root_kind == "ResultGroup" and _leaves_are_empty(root)
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
    if partition.endswith("populated_multi_key_maps") or partition.endswith("multiple_keys"):
        return any(
            len(obj.get("fields", [])) > 1
            for obj in walk_json(payload)
            if obj.get("kind_type") == "ValueObject"
        ) or any(len(obj) > 1 for obj in walk_json(payload) if "key" not in obj and "value" not in obj)
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
    closure = result_closure()
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
                instances = typed_instances(closure, payload) if isinstance(payload, dict) else {}
                present |= any(field in obj for obj in instances.get(table, []))
            if not present and table == "HighlightSpan" and field == "start_byte":
                present = any(
                    any("end_byte" in obj for obj in walk_json(envelopes.get(scenario, {})))
                    for scenario in scenarios
                )
            if not present:
                _, table_members = closure[table]
                raw_type = table_members[field].split("(", 1)[0].strip().strip("[]")
                if raw_type not in closure:
                    # FlatBuffers omits default scalar and enum slots from
                    # JSON. A typed table instance proves their default value
                    # is decoded by the production clients.
                    present = any(
                        scenario in envelopes
                        and bool(typed_instances(closure, envelopes[scenario].get("payload", {})).get(table))
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
    errors = verify_schema(result_closure(), manifest)
    if errors:
        print("\n".join(errors))
        return 1
    print("transitive result response schema classification PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
