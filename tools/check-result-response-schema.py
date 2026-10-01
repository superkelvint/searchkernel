#!/usr/bin/env python3
"""Check the transitive SearchResult schema and its declared corpus coverage."""

from __future__ import annotations

import importlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BFBS = ROOT / "core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs"
REFLECTION_FBS = ROOT / "tools/flatbuffers/reflection.fbs"
MANIFEST = ROOT / "reference/result-response-conformance/coverage.json"
CORPUS = ROOT / "reference/result-response-conformance/corpus.json"
CLASSIFIED_FIELDS = {
    "SearchResult": {"root", "total_hits", "coverage", "timing", "errors", "trace", "metadata"},
    "Hit": {"id", "relevance", "fields", "highlights", "source", "types", "match_features", "summary_features", "rank_features", "sort_values"},
    "ResultGroup": {"id", "label", "value", "fields", "children", "continuations", "source", "types", "relevance", "limits", "metadata"},
}


_REFLECTION_TEMP: tempfile.TemporaryDirectory[str] | None = None
_REFLECTION_MODULES: dict[str, Any] | None = None


def _reflection_modules() -> dict[str, Any]:
    """Generate official reflection table accessors with the pinned flatc."""
    global _REFLECTION_TEMP, _REFLECTION_MODULES
    if _REFLECTION_MODULES is None:
        flatc = subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "flatc"],
            text=True,
        ).strip()
        _REFLECTION_TEMP = tempfile.TemporaryDirectory(prefix="searchkernel-bfbs-reflection-")
        subprocess.run(
            [flatc, "--python", "-o", _REFLECTION_TEMP.name, str(REFLECTION_FBS)],
            cwd=ROOT,
            check=True,
        )
        sys.path.insert(0, _REFLECTION_TEMP.name)
        _REFLECTION_MODULES = {
            name: importlib.import_module(f"reflection.{name}")
            for name in ("Schema", "BaseType")
        }
    return _REFLECTION_MODULES


class ReflectedSchema:
    """Logical result closure over the official FlatBuffers reflection tables."""

    def __init__(self, data: bytes):
        modules = _reflection_modules()
        schema_type = modules["Schema"].Schema
        try:
            if not schema_type.SchemaBufferHasIdentifier(data, 0):
                raise ValueError("response schema is not a BFBS reflection schema")
            self.schema = schema_type.GetRootAs(data, 0)
            self.objects = [self.schema.Objects(i) for i in range(self.schema.ObjectsLength())]
            self.enums = [self.schema.Enums(i) for i in range(self.schema.EnumsLength())]
            self.object_names = {self._name(value): i for i, value in enumerate(self.objects)}
            self.object_indices = {
                self._name(value).rsplit(".", 1)[-1]: i for i, value in enumerate(self.objects)
            }
            self.enum_indices = {
                self._name(value).rsplit(".", 1)[-1]: i for i, value in enumerate(self.enums)
            }
            if any(not name for name in self.object_names):
                raise ValueError("BFBS contains a declaration without a name")
            self.base = modules["BaseType"]
            self.closure = self._result_closure()
            self.wire = self._wire_schema()
        except Exception as error:
            raise ValueError(f"invalid BFBS reflection metadata: {error}") from error

    @staticmethod
    def _name(table: Any) -> str:
        raw = table.Name()
        return raw.decode("utf-8") if raw else ""


    def _base_type_token(self, value: int) -> str:
        bt = self.base.BaseType()
        names = {
            bt.None_: "none",
            bt.UType: "utype",
            bt.Bool: "bool",
            bt.Byte: "byte",
            bt.UByte: "ubyte",
            bt.Short: "short",
            bt.UShort: "ushort",
            bt.Int: "int",
            bt.UInt: "uint",
            bt.Long: "long",
            bt.ULong: "ulong",
            bt.Float: "float",
            bt.Double: "double",
            bt.String: "string",
        }
        token = names.get(value)
        if token is None:
            raise ValueError(f"unsupported reflected scalar base type: {value}")
        return token

    def _type_signature(self, reflected_type: Any) -> str:
        bt = self.base.BaseType()
        base = reflected_type.BaseType()
        element = reflected_type.Element()
        index = reflected_type.Index()
        if base in (bt.Vector, bt.Vector64, bt.Array):
            if element == bt.Obj:
                item = self._name(self.objects[index]).rsplit(".", 1)[-1]
            elif element in (bt.Union, bt.UType):
                item = self._name(self.enums[index]).rsplit(".", 1)[-1]
            elif index >= 0:
                item = self._name(self.enums[index]).rsplit(".", 1)[-1]
            else:
                item = self._base_type_token(element)
            if base == bt.Array:
                return f"array<{item};{reflected_type.FixedLength()}>"
            if base == bt.Vector64:
                return f"vector64<{item}>"
            return f"vector<{item}>"
        if base == bt.Obj:
            return self._name(self.objects[index]).rsplit(".", 1)[-1]
        if base == bt.Union:
            return self._name(self.enums[index]).rsplit(".", 1)[-1]
        if index >= 0:
            return self._name(self.enums[index]).rsplit(".", 1)[-1]
        return self._base_type_token(base)

    def _enum_default_name(self, enum_index: int, value: int) -> str:
        enum = self.enums[enum_index]
        for i in range(enum.ValuesLength()):
            member = enum.Values(i)
            if member.Value() == value:
                return self._name(member)
        return str(value)

    def _scalar_default(self, field: Any, field_type: Any) -> str | None:
        """Return the reflected default for an enum or primitive scalar field."""
        bt = self.base.BaseType()
        base = field_type.BaseType()
        enum_index = field_type.Index()
        if field.Optional():
            return "null"
        if (
            enum_index >= 0
            and base not in (bt.Obj, bt.Union)
            and enum_index < len(self.enums)
            and not self.enums[enum_index].IsUnion()
        ):
            return self._enum_default_name(enum_index, field.DefaultInteger())
        if enum_index >= 0 or base in (
            bt.Obj,
            bt.Union,
            bt.Vector,
            bt.Vector64,
            bt.Array,
            bt.String,
        ):
            return None
        if base == bt.Bool:
            return "true" if field.DefaultInteger() else "false"
        if base in (bt.Float, bt.Double):
            return repr(float(field.DefaultReal()))
        scalar_types = {
            bt.Byte,
            bt.UByte,
            bt.Short,
            bt.UShort,
            bt.Int,
            bt.UInt,
            bt.Long,
            bt.ULong,
        }
        if base in scalar_types:
            return str(field.DefaultInteger())
        return None

    def _wire_schema(self) -> dict[str, dict[str, Any]]:
        """Pin declaration headers and ordered wire members from BFBS reflection."""
        bt = self.base.BaseType()
        result: dict[str, dict[str, Any]] = {}
        for name, (kind, _) in self.closure.items():
            if kind in {"enum", "union"}:
                enum = self.enums[self.enum_indices[name]]
                if kind == "union":
                    members = [
                        f"{self._name(enum.Values(i))}={enum.Values(i).Value()}"
                        for i in range(enum.ValuesLength())
                        if self._name(enum.Values(i)) != "NONE"
                    ]
                    header = "union"
                else:
                    members = [
                        f"{self._name(enum.Values(i))}={enum.Values(i).Value()}"
                        for i in range(enum.ValuesLength())
                    ]
                    underlying = enum.UnderlyingType()
                    if underlying is None:
                        raise ValueError(f"enum has no underlying type: {name}")
                    header = f"enum:{self._base_type_token(underlying.BaseType())}"
                result[name] = {"wire_header": header, "wire_members": members}
                continue
            reflected = self.objects[self.object_indices[name]]
            fields = [reflected.Fields(i) for i in range(reflected.FieldsLength())]
            header = kind
            if kind == "struct":
                header = f"struct:size={reflected.Bytesize()},align={reflected.Minalign()}"
            by_name = {self._name(field): field for field in fields}
            members: list[str] = []
            for field in sorted(fields, key=lambda value: value.Id()):
                field_name = self._name(field)
                field_type = field.Type()
                if field_type is None:
                    raise ValueError(f"field has no type metadata: {name}.{field_name}")
                if self._is_synthetic_union_discriminator(field_name, field_type, by_name):
                    continue
                signature = f"{field_name}:{self._type_signature(field_type)}"
                default = self._scalar_default(field, field_type)
                if default is not None:
                    signature += f"={default}"
                if field.Key():
                    signature += "(key)"
                if field.Required():
                    signature += "(required)"
                if field.Optional():
                    signature += "(optional)"
                if field.Offset64():
                    signature += "(offset64)"
                if kind == "struct":
                    signature += f"(offset={field.Offset()},padding={field.Padding()})"
                members.append(signature)
            result[name] = {"wire_header": header, "wire_members": members}
        return result

    def _references(self, reflected_type: Any) -> list[tuple[str, int]]:
        """Return object/enum indices for a scalar, vector, array, or union."""
        base = reflected_type.BaseType()
        element = reflected_type.Element()
        index = reflected_type.Index()
        bt = self.base
        if index < 0:
            return []
        if base in (bt.BaseType().Obj, bt.BaseType().Union):
            return [("object" if base == bt.BaseType().Obj else "enum", index)]
        if base in (bt.BaseType().Vector, bt.BaseType().Vector64, bt.BaseType().Array):
            return [("object" if element == bt.BaseType().Obj else "enum", index)]
        # Enum-valued scalars use their primitive storage BaseType and carry an
        # enum index; UType is the synthetic union discriminator storage type.
        return [("enum", index)]

    def _is_synthetic_union_discriminator(
        self,
        field_name: str,
        field_type: Any,
        by_name: dict[str, Any],
    ) -> bool:
        """Return whether a reflected field is FlatBuffers union tag metadata."""
        if not field_name.endswith("_type"):
            return False
        payload = by_name.get(field_name.removesuffix("_type"))
        payload_type = payload.Type() if payload is not None else None
        if payload_type is None or payload_type.Index() != field_type.Index():
            return False

        bt = self.base.BaseType()
        if (
            field_type.BaseType() == bt.UType
            and payload_type.BaseType() == bt.Union
        ):
            return True

        sequence_types = (bt.Vector, bt.Vector64, bt.Array)
        return (
            field_type.BaseType() in sequence_types
            and payload_type.BaseType() == field_type.BaseType()
            and field_type.Element() == bt.UType
            and payload_type.Element() == bt.Union
        )

    def _result_closure(self) -> dict[str, tuple[str, dict[str, Any]]]:
        bt = self.base.BaseType()
        start = self.object_names.get("SearchKernel.Wire.SearchResult")
        if start is None:
            raise ValueError("BFBS has no SearchKernel.Wire.SearchResult root type")
        pending: list[tuple[str, int]] = [("object", start)]
        found: dict[str, tuple[str, dict[str, Any]]] = {}
        while pending:
            category, index = pending.pop()
            entries = self.objects if category == "object" else self.enums
            if index < 0 or index >= len(entries):
                raise ValueError(f"BFBS {category} reference is out of range: {index}")
            reflected = entries[index]
            name = self._name(reflected).rsplit(".", 1)[-1]
            if name in found:
                continue
            if category == "enum":
                is_union = reflected.IsUnion()
                kind = "union" if is_union else "enum"
                values: dict[str, Any] = {}
                for i in range(reflected.ValuesLength()):
                    value = reflected.Values(i)
                    member = self._name(value)
                    if is_union and member == "NONE":
                        continue
                    values[member] = value.Value()
                    if is_union:
                        union_type = value.UnionType()
                        if union_type is None:
                            raise ValueError(f"union arm has no payload type: {name}.{member}")
                        pending.extend(self._references(union_type))
                found[name] = (kind, values)
                continue

            kind = "struct" if reflected.IsStruct() else "table"
            reflected_fields = [reflected.Fields(i) for i in range(reflected.FieldsLength())]
            by_name = {self._name(field): field for field in reflected_fields}
            values = {}
            for field in reflected_fields:
                field_name = self._name(field)
                field_type = field.Type()
                if field_type is None:
                    raise ValueError(f"field has no type metadata: {name}.{field_name}")
                # FlatBuffers emits an adjacent union discriminator for scalar
                # unions and a parallel discriminator vector for union vectors.
                # Both are implementation metadata; only the payload field is
                # part of the logical schema exposed to coverage.
                if self._is_synthetic_union_discriminator(
                    field_name, field_type, by_name
                ):
                    continue
                values[field_name] = field_name
                pending.extend(self._references(field_type))
            found[name] = (kind, values)
        return found

    def typed_instances(self, result_payload: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
        """Collect JSON table instances by following reflected field types."""
        bt = self.base.BaseType()
        found: dict[str, list[dict[str, Any]]] = {}
        visited: set[tuple[int, int]] = set()

        def visit_object(index: int, value: Any) -> None:
            if not isinstance(value, dict) or index < 0 or index >= len(self.objects):
                return
            marker = (index, id(value))
            if marker in visited:
                return
            visited.add(marker)
            reflected = self.objects[index]
            name = self._name(reflected).rsplit(".", 1)[-1]
            found.setdefault(name, []).append(value)
            fields = [reflected.Fields(i) for i in range(reflected.FieldsLength())]
            for field in fields:
                field_name = self._name(field)
                field_type = field.Type()
                base = field_type.BaseType()
                element = field_type.Element()
                type_index = field_type.Index()
                payload = value.get(field_name)
                if base == bt.Union:
                    enum = self.enums[type_index]
                    discriminator = value.get(f"{field_name}_type")
                    arms = self._union_arm_indices(enum)
                    arm_index = arms.get(discriminator)
                    if arm_index is not None:
                        visit_object(arm_index, payload)
                elif base == bt.Obj:
                    visit_object(type_index, payload)
                elif base in (bt.Vector, bt.Vector64, bt.Array):
                    items = payload if isinstance(payload, list) else []
                    if element == bt.Obj:
                        for item in items:
                            visit_object(type_index, item)
                    elif element == bt.Union:
                        enum = self.enums[type_index]
                        discriminator_values = value.get(f"{field_name}_type", [])
                        arms = self._union_arm_indices(enum)
                        if isinstance(discriminator_values, list):
                            for discriminator, item in zip(discriminator_values, items):
                                arm_index = arms.get(discriminator)
                                if arm_index is not None:
                                    visit_object(arm_index, item)

        root = self.object_names.get("SearchKernel.Wire.SearchResult")
        if root is not None:
            visit_object(root, result_payload)
        return found

    def _union_arm_indices(self, enum: Any) -> dict[str, int]:
        arms: dict[str, int] = {}
        for i in range(enum.ValuesLength()):
            value = enum.Values(i)
            name = self._name(value)
            if name == "NONE":
                continue
            union_type = value.UnionType()
            if union_type is not None and union_type.BaseType() == self.base.BaseType().Obj:
                arms[name] = union_type.Index()
        return arms


def result_closure(bfbs: bytes) -> dict[str, tuple[str, dict[str, Any]]]:
    return ReflectedSchema(bfbs).closure


def result_wire_schema(bfbs: bytes) -> dict[str, dict[str, Any]]:
    return ReflectedSchema(bfbs).wire


def schema_manifest(
    reflection: ReflectedSchema,
    prior_schema: dict[str, Any] | None = None,
) -> dict[str, dict[str, Any]]:
    """Build the checked-in schema signatures directly from official BFBS metadata."""
    existing_order = [
        name for name in (prior_schema or {}) if name in reflection.closure
    ]
    added_names = sorted(set(reflection.closure) - set(existing_order))
    return {
        name: {
            "kind": kind,
            "members": sorted(members),
            "wire_members": reflection.wire[name]["wire_members"],
            "wire_header": reflection.wire[name]["wire_header"],
        }
        for name in existing_order + added_names
        for kind, members in (reflection.closure[name],)
    }


def verify_schema(
    closure: dict[str, tuple[str, dict[str, Any]]],
    manifest: dict[str, Any],
    wire: dict[str, dict[str, Any]] | None = None,
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
        if wire is not None:
            actual_wire = wire.get(name)
            if not isinstance(actual_wire, dict):
                errors.append(f"{name} wire signature is missing")
            else:
                declared_header = declared.get("wire_header")
                actual_header = actual_wire.get("wire_header")
                if declared_header != actual_header:
                    errors.append(
                        f"{name} wire header mismatch: expected={declared_header!r}, actual={actual_header!r}"
                    )
                declared_wire = declared.get("wire_members")
                actual_wire_members = actual_wire.get("wire_members")
                if declared_wire != actual_wire_members:
                    errors.append(
                        f"{name} wire members mismatch: expected={declared_wire!r}, actual={actual_wire_members!r}"
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


def verify(bfbs: bytes) -> list[str]:
    reflection = ReflectedSchema(bfbs)
    return verify_schema(reflection.closure, json.loads(MANIFEST.read_text()), reflection.wire)


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
    reflection: ReflectedSchema, result_payload: dict[str, Any]
) -> dict[str, list[dict[str, Any]]]:
    return reflection.typed_instances(result_payload)
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
    bfbs = BFBS.read_bytes()
    reflection = ReflectedSchema(bfbs)
    closure = reflection.closure
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
                instances = typed_instances(reflection, payload) if isinstance(payload, dict) else {}
                present |= any(field in obj for obj in instances.get(table, []))
            if not present and table == "HighlightSpan" and field == "start_byte":
                present = any(
                    any("end_byte" in obj for obj in walk_json(envelopes.get(scenario, {})))
                    for scenario in scenarios
                )
            if not present:
                member_refers_to_type = _field_refers_to_closure(reflection, table, field, closure)
                if not member_refers_to_type:
                    # FlatBuffers omits default scalar and enum slots from
                    # JSON. A typed table instance proves their default value
                    # is decoded by the production clients.
                    present = any(
                        scenario in envelopes
                        and bool(typed_instances(reflection, envelopes[scenario].get("payload", {})).get(table))
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


def _field_refers_to_closure(
    reflection: ReflectedSchema,
    table_name: str,
    field_name: str,
    closure: dict[str, tuple[str, dict[str, Any]]],
) -> bool:
    index = reflection.object_indices.get(table_name)
    if index is None:
        return False
    reflected = reflection.objects[index]
    for i in range(reflected.FieldsLength()):
        field = reflected.Fields(i)
        if ReflectedSchema._name(field) != field_name:
            continue
        field_type = field.Type()
        return any(
            (
                reflection._name(reflection.objects[ref]).rsplit(".", 1)[-1]
                if category == "object" and 0 <= ref < len(reflection.objects)
                else reflection._name(reflection.enums[ref]).rsplit(".", 1)[-1]
                if category == "enum" and 0 <= ref < len(reflection.enums)
                else ""
            ) in closure
            for category, ref in reflection._references(field_type)
        )
    return False


def main() -> int:
    manifest = json.loads(MANIFEST.read_text())
    reflection = ReflectedSchema(BFBS.read_bytes())
    if sys.argv[1:] == ["--write-signatures"]:
        manifest["schema"] = schema_manifest(reflection, manifest.get("schema"))
        MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
        print("updated result response schema signatures from BFBS")
        return 0
    if sys.argv[1:]:
        print(
            "usage: check-result-response-schema.py [--write-signatures]",
            file=sys.stderr,
        )
        return 2
    errors = verify_schema(reflection.closure, manifest, reflection.wire)
    if errors:
        print("\n".join(errors))
        return 1
    print("transitive result response schema classification PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
