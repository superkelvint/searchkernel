#!/usr/bin/env python3
"""Regression tests for BFBS-derived semantic model metadata."""

from __future__ import annotations

import unittest
from pathlib import Path

from searchkernel_bfbs import ReflectedSchema

ROOT = Path(__file__).resolve().parents[1]
BFBS = ROOT / "core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs"


class SearchKernelBfbsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.schema = ReflectedSchema(BFBS)

    def field(self, table: str, name: str):
        for field in self.schema.fields(self.schema.object(table)):
            if self.schema.field_name(field) == name:
                return field
        self.fail(f"{table}.{name} is missing from BFBS")

    def test_scalar_string_vector_and_nested_table_shapes(self) -> None:
        self.assertEqual(self.schema.ts_type(self.field("Document", "id")), "string|Uint8Array|null")
        self.assertEqual(self.schema.ts_default(self.field("Document", "id")), "null")
        self.assertEqual(self.schema.ts_type(self.field("Document", "fields")), "(ValueEntryT)[]")
        self.assertEqual(self.schema.ts_default(self.field("Document", "fields")), "[]")
        self.assertEqual(self.schema.ts_type(self.field("OptionalBool", "value")), "boolean")
        self.assertEqual(self.schema.ts_default(self.field("OptionalBool", "value")), "false")
        self.assertEqual(self.schema.ts_type(self.field("TotalHits", "value")), "bigint")
        self.assertEqual(self.schema.ts_default(self.field("TotalHits", "value")), "BigInt('0')")

    def test_enum_and_union_shapes_are_reflected(self) -> None:
        kind_type = self.field("Value", "kindType")
        kind = self.field("Value", "kind")
        self.assertEqual(self.schema.ts_type(kind_type), "ValueKind")
        self.assertEqual(self.schema.enum_spec(kind_type), "ValueKind")
        union_type = self.schema.ts_type(kind)
        self.assertIn("Int64ValueT", union_type)
        self.assertIn("TensorValueT", union_type)
        self.assertTrue(union_type.endswith("|null"))
        self.assertEqual(self.schema.ts_default(kind_type), "ValueKind.NONE")
        self.assertEqual(self.schema.ts_default(kind), "null")

    def test_wire_declarations_are_version_neutral(self) -> None:
        self.assertTrue(self.schema.wire_objects())
        self.assertTrue(self.schema.wire_enums())
        self.assertTrue(all(".V1." not in item.Name().decode() for item in self.schema.wire_objects()))
        self.assertTrue(all(".V2." not in item.Name().decode() for item in self.schema.wire_objects()))


if __name__ == "__main__":
    unittest.main()
