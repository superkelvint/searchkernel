from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tools/check-result-response-schema.py"
SPEC = importlib.util.spec_from_file_location("result_response_schema", SOURCE)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ResultResponseSchemaTest(unittest.TestCase):
    MINIMAL_SCHEMA = """namespace SearchKernel.Wire;

enum Relation:ubyte {
  Unspecified = 0,
  Exact = 1
}

struct FixedValues {
  values:[int:4];
}

struct LayoutValues {
  head:ubyte;
  tail:uint;
}

table SearchResult {
  bool_value:bool = false;
  int_value:long = 0;
  double_value:double = 0.0;
  relation:Relation = Unspecified;
  normal:[int];
  wide:[int] (vector64);
  offset:[int] (offset64);
  fixed:FixedValues;
  layout:LayoutValues;
  presence:bool = null;
  required_text:string (required);
  key_text:string (key);
}

root_type SearchResult;
"""

    @staticmethod
    def compile_bfbs(schema: str, directory: Path, name: str) -> bytes:
        flatc = subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "flatc"],
            text=True,
        ).strip()
        source = directory / f"{name}.fbs"
        source.write_text(schema)
        subprocess.run(
            [flatc, "--binary", "--schema", "-o", str(directory), str(source)],
            check=True,
            cwd=ROOT,
        )
        return (directory / f"{name}.bfbs").read_bytes()

    @staticmethod
    def manifest_for(bfbs: bytes) -> dict:
        reflection = MODULE.ReflectedSchema(bfbs)
        schema = {
            name: {
                "kind": kind,
                "members": list(members),
                **reflection.wire[name],
            }
            for name, (kind, members) in reflection.closure.items()
        }
        tables = {
            name: {field: [] for field in members}
            for name, (kind, members) in reflection.closure.items()
            if kind in {"table", "struct"}
        }
        return {"schema": schema, "partitions": {"fields": tables}}

    def assert_extracted_drift(
        self,
        baseline: bytes,
        changed: bytes,
        expected: str,
    ) -> None:
        changed_reflection = MODULE.ReflectedSchema(changed)
        changed_type = next(
            name
            for name, declaration in changed_reflection.wire.items()
            if any(expected in signature for signature in declaration["wire_members"])
        )
        errors = MODULE.verify_schema(
            changed_reflection.closure,
            self.manifest_for(baseline),
            changed_reflection.wire,
        )
        self.assertTrue(
            any(
                error.startswith(f"{changed_type} wire members mismatch:")
                for error in errors
            ),
            errors,
        )

    def test_bfbs_extraction_pins_primitive_defaults(self) -> None:
        cases = (
            ("bool_value:bool = false;", "bool_value:bool = true;", "bool_value:bool=true"),
            ("int_value:long = 0;", "int_value:long = 1;", "int_value:long=1"),
            (
                "double_value:double = 0.0;",
                "double_value:double = 1.0;",
                "double_value:double=1.0",
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "defaults_base")
            for index, (old, new, expected) in enumerate(cases):
                with self.subTest(expected=expected):
                    schema = self.MINIMAL_SCHEMA.replace(old, new)
                    changed = self.compile_bfbs(
                        schema, directory, f"defaults_changed_{index}"
                    )
                    self.assert_extracted_drift(baseline, changed, expected)

    def test_bfbs_extraction_keeps_enum_default_symbolic_and_numeric_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "enum_base")
            baseline_reflection = MODULE.ReflectedSchema(baseline)
            self.assertIn(
                "relation:Relation=Unspecified",
                baseline_reflection.wire["SearchResult"]["wire_members"],
            )
            changed_schema = self.MINIMAL_SCHEMA.replace(
                "Unspecified = 0,", "Unspecified = 4,"
            )
            changed = self.compile_bfbs(changed_schema, directory, "enum_changed")
            changed_reflection = MODULE.ReflectedSchema(changed)
            self.assertEqual(
                baseline_reflection.wire["Relation"]["wire_header"], "enum:ubyte"
            )
            self.assertIn(
                "Unspecified=4", changed_reflection.wire["Relation"]["wire_members"]
            )
            errors = MODULE.verify_schema(
                changed_reflection.closure,
                self.manifest_for(baseline),
                changed_reflection.wire,
            )
            self.assertTrue(
                any("Relation wire members mismatch" in error for error in errors)
            )


    def test_bfbs_extraction_pins_enum_storage_type(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "enum_storage_base")
            changed_schema = self.MINIMAL_SCHEMA.replace(
                "enum Relation:ubyte {", "enum Relation:ushort {"
            )
            changed = self.compile_bfbs(
                changed_schema, directory, "enum_storage_changed"
            )
            baseline_reflection = MODULE.ReflectedSchema(baseline)
            changed_reflection = MODULE.ReflectedSchema(changed)
            self.assertEqual(
                baseline_reflection.wire["Relation"]["wire_header"], "enum:ubyte"
            )
            self.assertEqual(
                changed_reflection.wire["Relation"]["wire_header"], "enum:ushort"
            )
            errors = MODULE.verify_schema(
                changed_reflection.closure,
                self.manifest_for(baseline),
                changed_reflection.wire,
            )
            self.assertTrue(
                any("Relation wire header mismatch" in error for error in errors),
                errors,
            )

    def test_bfbs_extraction_distinguishes_vector_and_vector64(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "vector_base")
            baseline_reflection = MODULE.ReflectedSchema(baseline)
            self.assertIn(
                "normal:vector<int>=null(optional)",
                baseline_reflection.wire["SearchResult"]["wire_members"],
            )
            changed_schema = self.MINIMAL_SCHEMA.replace(
                "normal:[int];", "normal:[int] (vector64);"
            )
            changed = self.compile_bfbs(changed_schema, directory, "vector64_changed")
            self.assert_extracted_drift(
                baseline,
                changed,
                "normal:vector64<int>=null(optional)(offset64)",
            )

    def test_bfbs_extraction_pins_fixed_array_element_and_length(self) -> None:
        cases = (
            ("values:[int:4];", "values:[int:5];", "values:array<int;5>"),
            ("values:[int:4];", "values:[long:4];", "values:array<long;4>"),
        )
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "array_base")
            for index, (old, new, expected) in enumerate(cases):
                with self.subTest(expected=expected):
                    changed_schema = self.MINIMAL_SCHEMA.replace(old, new)
                    changed = self.compile_bfbs(
                        changed_schema, directory, f"array_changed_{index}"
                    )
                    self.assert_extracted_drift(baseline, changed, expected)


    def test_bfbs_extraction_pins_struct_layout_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "struct_layout_base")
            baseline_reflection = MODULE.ReflectedSchema(baseline)
            self.assertEqual(
                baseline_reflection.wire["FixedValues"]["wire_header"],
                "struct:size=16,align=4",
            )
            self.assertEqual(
                baseline_reflection.wire["LayoutValues"]["wire_header"],
                "struct:size=8,align=4",
            )
            self.assertIn(
                "head:ubyte=0(offset=0,padding=3)",
                baseline_reflection.wire["LayoutValues"]["wire_members"],
            )
            self.assertIn(
                "tail:uint=0(offset=4,padding=0)",
                baseline_reflection.wire["LayoutValues"]["wire_members"],
            )

            changed_schema = self.MINIMAL_SCHEMA.replace(
                "struct FixedValues {", "struct FixedValues (force_align: 16) {"
            )
            changed = self.compile_bfbs(
                changed_schema, directory, "struct_layout_changed"
            )
            changed_reflection = MODULE.ReflectedSchema(changed)
            self.assertEqual(
                changed_reflection.wire["FixedValues"]["wire_header"],
                "struct:size=16,align=16",
            )
            errors = MODULE.verify_schema(
                changed_reflection.closure,
                self.manifest_for(baseline),
                changed_reflection.wire,
            )
            self.assertTrue(
                any("FixedValues wire header mismatch" in error for error in errors),
                errors,
            )

    def test_bfbs_extraction_pins_presence_and_offset_flags(self) -> None:
        cases = (
            ("presence:bool = null;", "presence:bool = false;", "presence:bool=false"),
            (
                "offset:[int] (offset64);",
                "offset:[int];",
                "offset:vector<int>=null(optional)",
            ),
            (
                "required_text:string (required);",
                "required_text:string;",
                "required_text:string=null(optional)",
            ),
            (
                "key_text:string (key);",
                "key_text:string;",
                "key_text:string=null(optional)",
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = self.compile_bfbs(self.MINIMAL_SCHEMA, directory, "flags_base")
            baseline_members = MODULE.ReflectedSchema(baseline).wire[
                "SearchResult"
            ]["wire_members"]
            self.assertIn("presence:bool=null(optional)", baseline_members)
            self.assertIn("wide:vector64<int>=null(optional)(offset64)", baseline_members)
            self.assertIn("offset:vector<int>=null(optional)(offset64)", baseline_members)
            self.assertIn("required_text:string(required)", baseline_members)
            self.assertIn("key_text:string(key)(required)", baseline_members)
            for index, (old, new, expected) in enumerate(cases):
                with self.subTest(expected=expected):
                    changed_schema = self.MINIMAL_SCHEMA.replace(old, new)
                    changed = self.compile_bfbs(
                        changed_schema, directory, f"flags_changed_{index}"
                    )
                    self.assert_extracted_drift(baseline, changed, expected)

    def test_checked_in_response_bfbs_is_the_schema_authority(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        self.assertEqual(MODULE.verify(MODULE.BFBS.read_bytes()), [])
        self.assertIn("TensorCell", closure)
        self.assertIn("ValueKind", closure)

        wire = MODULE.result_wire_schema(MODULE.BFBS.read_bytes())
        self.assertIn(
            "relation:TotalHitsRelation=Unspecified",
            wire["TotalHits"]["wire_members"],
        )

    def test_checked_in_signatures_pin_nullable_table_and_vector_defaults(self) -> None:
        manifest = json.loads(MODULE.MANIFEST.read_text())
        declared = manifest["schema"]
        self.assertIn(
            "root:ResultNode=null(optional)",
            declared["SearchResult"]["wire_members"],
        )
        self.assertIn(
            "fields:vector<ValueEntry>=null(optional)",
            declared["ValueObject"]["wire_members"],
        )

    def test_corrupt_non_bfbs_input_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            MODULE.result_closure(b"not a BFBS reflection schema")

    def test_synthetic_union_discriminators_are_not_logical_fields(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        self.assertEqual(set(closure["ResultNode"][1]), {"node"})
        self.assertEqual(set(closure["Value"][1]), {"kind"})
        self.assertNotIn("NONE", closure["ResultNodeKind"][1])
        self.assertEqual(set(closure["ResultNodeKind"][1]), {"Hit", "ResultGroup"})

    def test_vector_union_discriminator_is_not_a_logical_field(self) -> None:
        schema = """namespace SearchKernel.Wire;

table Hit {
  id:string;
}

table ResultGroup {
  id:string;
}

union ResultNodeKind {
  Hit,
  ResultGroup
}

table SearchResult {
  nodes:[ResultNodeKind];
}

root_type SearchResult;
"""
        flatc = subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "flatc"],
            text=True,
        ).strip()
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "vector_union.fbs"
            source.write_text(schema)
            subprocess.run(
                [
                    flatc,
                    "--binary",
                    "--schema",
                    "-o",
                    directory,
                    str(source),
                ],
                check=True,
                cwd=ROOT,
            )
            closure = MODULE.result_closure(
                (Path(directory) / "vector_union.bfbs").read_bytes()
            )

        self.assertEqual(set(closure["SearchResult"][1]), {"nodes"})
        self.assertNotIn("nodes_type", closure["SearchResult"][1])
        self.assertEqual(
            set(closure["ResultNodeKind"][1]),
            {"Hit", "ResultGroup"},
        )

    def test_new_hit_field_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["Hit"][1]["future_result_field"] = "future_result_field"
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("Hit members mismatch" in error for error in errors))

    def test_new_reachable_result_type_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["FutureResultMetadata"] = ("table", {})
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("result schema declarations mismatch" in error for error in errors))

    def test_new_deeply_nested_field_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["TensorCell"][1]["future_cell_field"] = "future_cell_field"
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("TensorCell members mismatch" in error for error in errors))

    def test_new_total_hits_relation_value_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["TotalHitsRelation"][1]["FutureRelation"] = 5
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("TotalHitsRelation members mismatch" in error for error in errors))


    def test_field_type_change_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        index = wire["Hit"]["wire_members"].index("fields:vector<ValueEntry>=null(optional)")
        wire["Hit"]["wire_members"][index] = "fields:vector<StringEntry>=null(optional)"
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("Hit wire members mismatch" in error for error in errors))

    def test_field_reordering_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        wire["Hit"]["wire_members"] = list(reversed(wire["Hit"]["wire_members"]))
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("Hit wire members mismatch" in error for error in errors))

    def test_enum_storage_type_change_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        wire["TotalHitsRelation"]["wire_header"] = "enum:ushort"
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("TotalHitsRelation wire header mismatch" in error for error in errors))

    def test_enum_numeric_identity_change_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        members = wire["TotalHitsRelation"]["wire_members"]
        members[members.index("Unknown=4")] = "Unknown=9"
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("TotalHitsRelation wire members mismatch" in error for error in errors))

    def test_scalar_default_change_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        members = wire["TotalHits"]["wire_members"]
        members[members.index("value:ulong=0")] = "value:ulong=7"
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("TotalHits wire members mismatch" in error for error in errors))

    def test_union_numeric_identity_change_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        members = wire["ResultNodeKind"]["wire_members"]
        members[members.index("ResultGroup=2")] = "ResultGroup=9"
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("ResultNodeKind wire members mismatch" in error for error in errors))

    def test_union_reordering_fails(self) -> None:
        closure = MODULE.result_closure(MODULE.BFBS.read_bytes())
        wire = copy.deepcopy(MODULE.result_wire_schema(MODULE.BFBS.read_bytes()))
        wire["ResultNodeKind"]["wire_members"] = ["ResultGroup=2", "Hit=1"]
        errors = MODULE.verify_schema(closure, self.manifest(), wire)
        self.assertTrue(any("ResultNodeKind wire members mismatch" in error for error in errors))

    def test_new_result_node_union_arm_fails_until_coverage_is_updated(self) -> None:
        closure = copy.deepcopy(MODULE.result_closure(MODULE.BFBS.read_bytes()))
        closure["ResultNodeKind"][1]["FutureNode"] = 3
        errors = MODULE.verify_schema(closure, self.manifest())
        self.assertTrue(any("ResultNodeKind members mismatch" in error for error in errors))

    @staticmethod
    def manifest():
        return MODULE.json.loads(MODULE.MANIFEST.read_text())

    def test_classified_field_must_be_populated_by_its_scenario(self) -> None:
        envelope = {
            "payload": {
                "root": {
                    "node_type": "ResultGroup",
                    "node": {
                        "id": "root",
                        "label": "Root",
                        "value": {},
                        "fields": [],
                        "children": [
                            {
                                "node_type": "Hit",
                                "node": {
                                    field: object()
                                    for field in MODULE.CLASSIFIED_FIELDS["Hit"]
                                    if field != "highlights"
                                },
                            }
                        ],
                        "continuations": [],
                        "source": "source",
                        "types": [],
                        "relevance": {},
                        "limits": {},
                        "metadata": [],
                    },
                },
                "total_hits": {},
                "coverage": {},
                "timing": {},
                "errors": [],
                "trace": {},
                "metadata": [],
            }
        }
        errors = MODULE.verify_fixture_coverage({"canonical": envelope})
        self.assertIn(
            "Hit.highlights is not populated by scenario canonical",
            errors,
        )

    def test_manifest_scenario_must_really_populate_field(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["fields"]["Hit"]["highlights"] = ["empty"]
        errors = MODULE.verify_partitions(manifest, {"empty": {"payload": {}}})
        self.assertTrue(any("Hit.highlights is not exercised" in error for error in errors))

    def test_map_partition_requires_an_empty_map_fixture(self) -> None:
        manifest = json.loads(MODULE.MANIFEST.read_text())

        def value_object(keys: list[str]) -> dict:
            return {
                "kind_type": "ValueObject",
                "kind": {"fields": [{"key": key} for key in keys]},
            }

        envelopes = {
            "empty": {"payload": {}},
            "canonical": {"payload": {"value": value_object(["count"])}},
            "value-variants": {
                "payload": {
                    "values": [
                        value_object([]),
                        value_object(["null", "text"]),
                    ]
                }
            },
        }
        errors = MODULE.verify_partitions(manifest, envelopes)
        self.assertFalse(
            any(
                "maps.empty_populated_and_multiple_keys has no scenario" in error
                for error in errors
            ),
            errors,
        )

    def test_malformed_manifest_scenario_must_match_failure_class(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["semantic"]["malformed.unknown_value_union"] = [
            "unknown-result-node-union"
        ]
        errors = MODULE.verify_partitions(manifest, {})
        self.assertTrue(
            any(
                "malformed.unknown_value_union scenario unknown-result-node-union"
                in error
                for error in errors
            )
        )

    def test_manifest_missing_scenario_fails(self) -> None:
        manifest = self.manifest()
        manifest["partitions"]["semantic"]["test.missing"] = ["does-not-exist"]
        errors = MODULE.verify_partitions(manifest, {})
        self.assertIn("test.missing references missing scenario does-not-exist", errors)

    def test_trace_shape_requires_every_classified_field(self) -> None:
        trace = {
            field: None for field in MODULE.CLASSIFIED_FIELDS["SearchResult"]
        }
        trace["root"] = {
            "kind": "hit",
            **{
                field: None
                for field in MODULE.CLASSIFIED_FIELDS["Hit"]
                if field != "sort_values"
            },
        }
        errors = MODULE.trace_shape_errors(trace)
        self.assertTrue(any("sort_values" in error for error in errors))

    def test_trace_shape_checks_transitive_result_subobjects(self) -> None:
        trace = {
            "root": {
                "kind": "hit",
                **{field: {} if field in {"fields", "match_features", "summary_features", "rank_features"} else [] if field in {"highlights", "types", "sort_values"} else None for field in MODULE.CLASSIFIED_FIELDS["Hit"]},
            },
            "total_hits": {"value": "1", "relation": "exact"},
            "coverage": {"metadata": {}},
            "timing": None,
            "errors": [],
            "trace": None,
            "metadata": {},
        }
        errors = MODULE.trace_shape_errors(trace)
        self.assertTrue(any("coverage fields mismatch" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
