#!/usr/bin/env python3
"""Shared BFBS reflection helpers for Search Kernel code generation."""

from __future__ import annotations

import importlib
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REFLECTION_FBS = ROOT / "tools/flatbuffers/reflection.fbs"
WIRE_PREFIX = "SearchKernel.Wire."

_REFLECTION_TEMP: tempfile.TemporaryDirectory[str] | None = None
_REFLECTION_MODULES: dict[str, Any] | None = None


def _reflection_modules() -> dict[str, Any]:
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


def _decode(raw: bytes | None) -> str:
    return raw.decode("utf-8") if raw else ""


def _camel(name: str) -> str:
    head, *tail = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in tail)


def _normalize_union_member(name: str) -> str:
    return name.removeprefix("SearchKernel_Wire_")


class ReflectedSchema:
    """Typed view over the canonical Search Kernel BFBS schema."""

    def __init__(self, path: Path):
        modules = _reflection_modules()
        schema_type = modules["Schema"].Schema
        data = path.read_bytes()
        if not schema_type.SchemaBufferHasIdentifier(data, 0):
            raise ValueError(f"{path} is not a BFBS reflection schema")
        self.schema = schema_type.GetRootAs(data, 0)
        self.base = modules["BaseType"].BaseType()
        self.objects = [self.schema.Objects(i) for i in range(self.schema.ObjectsLength())]
        self.enums = [self.schema.Enums(i) for i in range(self.schema.EnumsLength())]
        self.object_by_name = {_decode(item.Name()): item for item in self.objects}
        self.enum_by_name = {_decode(item.Name()): item for item in self.enums}

    @staticmethod
    def short_name(item: Any) -> str:
        return _decode(item.Name()).rsplit(".", 1)[-1]

    def wire_objects(self) -> list[Any]:
        return sorted(
            (item for item in self.objects if _decode(item.Name()).startswith(WIRE_PREFIX)),
            key=self.short_name,
        )

    def wire_enums(self) -> list[Any]:
        return sorted(
            (item for item in self.enums if _decode(item.Name()).startswith(WIRE_PREFIX)),
            key=self.short_name,
        )

    @staticmethod
    def fields(obj: Any) -> list[Any]:
        return sorted((obj.Fields(i) for i in range(obj.FieldsLength())), key=lambda field: field.Id())

    def field_name(self, field: Any) -> str:
        return _camel(_decode(field.Name()))

    def _enum(self, index: int) -> Any:
        if index < 0 or index >= len(self.enums):
            raise ValueError(f"invalid BFBS enum index: {index}")
        return self.enums[index]

    def _object(self, index: int) -> Any:
        if index < 0 or index >= len(self.objects):
            raise ValueError(f"invalid BFBS object index: {index}")
        return self.objects[index]

    def enum_values(self, enum: Any) -> list[tuple[str, int]]:
        values: list[tuple[str, int]] = []
        for i in range(enum.ValuesLength()):
            item = enum.Values(i)
            values.append((_normalize_union_member(_decode(item.Name())), int(item.Value())))
        return values

    def _enum_name(self, index: int) -> str:
        return self.short_name(self._enum(index))

    def _object_type(self, index: int) -> str:
        obj = self._object(index)
        if obj.IsStruct():
            raise ValueError(f"semantic model does not support FlatBuffers structs: {self.short_name(obj)}")
        return f"{self.short_name(obj)}T"

    def _union_type(self, enum_index: int) -> str:
        enum = self._enum(enum_index)
        if not enum.IsUnion():
            raise ValueError(f"expected union enum, got {self.short_name(enum)}")
        types: list[str] = []
        for i in range(enum.ValuesLength()):
            member = enum.Values(i)
            union_type = member.UnionType()
            base = union_type.BaseType()
            if base == self.base.None_:
                continue
            if base != self.base.Obj:
                raise ValueError(
                    f"unsupported union member {self.short_name(enum)}.{_decode(member.Name())}: base type {base}"
                )
            item = self._object_type(union_type.Index())
            if item not in types:
                types.append(item)
        return "|".join(types + ["null"])

    def _scalar_type(self, base: int, index: int, *, vector_element: bool = False) -> str:
        if index >= 0 and base not in (self.base.Obj, self.base.Union):
            return self._enum_name(index)
        if base == self.base.Bool:
            return "boolean"
        if base in (
            self.base.Byte,
            self.base.UByte,
            self.base.Short,
            self.base.UShort,
            self.base.Int,
            self.base.UInt,
            self.base.Float,
            self.base.Double,
        ):
            return "number"
        if base in (self.base.Long, self.base.ULong):
            return "bigint"
        if base == self.base.String:
            return "string" if vector_element else "string|Uint8Array|null"
        if base == self.base.Obj:
            return self._object_type(index)
        if base == self.base.Union:
            return self._union_type(index)
        if base == self.base.UType:
            return self._enum_name(index)
        raise ValueError(f"unsupported reflected base type: {base}")

    def ts_type(self, field: Any) -> str:
        reflected = field.Type()
        base = reflected.BaseType()
        index = reflected.Index()
        if base in (self.base.Vector, self.base.Vector64):
            item = self._scalar_type(reflected.Element(), index, vector_element=True)
            return f"({item})[]"
        if base == self.base.Array:
            raise ValueError(f"semantic model does not support fixed arrays: {_decode(field.Name())}")
        result = self._scalar_type(base, index)
        if field.Optional() and "null" not in result.split("|"):
            result += "|null"
        if base == self.base.Obj and "|null" not in result:
            result += "|null"
        return result

    def _enum_default(self, enum_index: int, value: int) -> str:
        enum = self._enum(enum_index)
        name = self.short_name(enum)
        for member_name, member_value in self.enum_values(enum):
            if member_value == value:
                return f"{name}.{member_name}"
        raise ValueError(f"enum {name} has no value {value}")

    def ts_default(self, field: Any) -> str:
        reflected = field.Type()
        base = reflected.BaseType()
        index = reflected.Index()
        if field.Optional():
            return "null"
        if base in (self.base.Vector, self.base.Vector64):
            return "[]"
        if base == self.base.Array:
            raise ValueError(f"semantic model does not support fixed arrays: {_decode(field.Name())}")
        if base in (self.base.String, self.base.Obj, self.base.Union):
            return "null"
        if index >= 0:
            return self._enum_default(index, int(field.DefaultInteger()))
        if base == self.base.Bool:
            return "true" if field.DefaultInteger() else "false"
        if base in (self.base.Float, self.base.Double):
            return repr(float(field.DefaultReal()))
        if base in (self.base.Long, self.base.ULong):
            return f"BigInt('{int(field.DefaultInteger())}')"
        if base in (
            self.base.Byte,
            self.base.UByte,
            self.base.Short,
            self.base.UShort,
            self.base.Int,
            self.base.UInt,
        ):
            return str(int(field.DefaultInteger()))
        raise ValueError(f"unsupported default for base type {base}: {_decode(field.Name())}")

    def enum_spec(self, field: Any) -> str | None:
        reflected = field.Type()
        base = reflected.BaseType()
        index = reflected.Index()
        if base in (self.base.Vector, self.base.Vector64):
            element = reflected.Element()
            if index >= 0 and element not in (self.base.Obj, self.base.Union):
                return f"{self._enum_name(index)}[]"
            return None
        if index >= 0 and base not in (self.base.Obj, self.base.Union):
            return self._enum_name(index)
        return None

    def object(self, short_name: str) -> Any:
        return self.object_by_name[f"{WIRE_PREFIX}{short_name}"]

    def enum(self, short_name: str) -> Any:
        return self.enum_by_name[f"{WIRE_PREFIX}{short_name}"]
