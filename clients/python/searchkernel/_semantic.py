"""Version-free objects exchanged by the Python client and private codecs."""

from __future__ import annotations

from typing import Any


class SemanticEnumValue:
    """Stable symbolic enum identity owned by the semantic model, not a wire codec."""

    __slots__ = ("enum_name", "member_name")

    def __init__(self, enum_name: str, member_name: str) -> None:
        self.enum_name = enum_name
        self.member_name = member_name

    def __eq__(self, other: object) -> bool:
        return isinstance(other, SemanticEnumValue) and (
            self.enum_name,
            self.member_name,
        ) == (
            other.enum_name,
            other.member_name,
        )

    def __hash__(self) -> int:
        return hash((self.enum_name, self.member_name))

    def __repr__(self) -> str:
        return f"{self.enum_name}.{self.member_name}"


class _EnumNamespace:
    __slots__ = ("name",)

    def __init__(self, name: str) -> None:
        self.name = name

    def __getattr__(self, member: str) -> SemanticEnumValue:
        return SemanticEnumValue(self.name, member)


class SemanticTable:
    """Owned table-shaped data whose identity is independent of FlatBuffers."""

    __slots__ = ("_schema_name", "_values", "_positional")

    def __init__(self, schema_name: str, *args: Any, **kwargs: Any) -> None:
        object.__setattr__(self, "_schema_name", schema_name)
        object.__setattr__(self, "_values", dict(kwargs))
        object.__setattr__(self, "_positional", args)

    def __getattr__(self, name: str) -> Any:
        try:
            return self._values[name]
        except KeyError as exc:
            raise AttributeError(name) from exc

    def __setattr__(self, name: str, value: Any) -> None:
        if name in SemanticTable.__slots__:
            object.__setattr__(self, name, value)
        else:
            self._values[name] = value

    def __repr__(self) -> str:
        return f"SemanticTable({self._schema_name!r}, {self._values!r})"


class _TableFactory:
    __slots__ = ("name",)

    def __init__(self, name: str) -> None:
        self.name = name

    def __call__(self, *args: Any, **kwargs: Any) -> SemanticTable:
        return SemanticTable(self.name, *args, **kwargs)


def table_type(name: str) -> _TableFactory:
    return _TableFactory(name)


def is_semantic_table(value: object, name: str) -> bool:
    return (isinstance(value, SemanticTable) and value._schema_name == name) or (
        value.__class__.__name__ in {name, f"{name}T"}
    )


def __getattr__(name: str) -> Any:
    if name.endswith("T"):
        return _TableFactory(name[:-1])
    return _EnumNamespace(name)
