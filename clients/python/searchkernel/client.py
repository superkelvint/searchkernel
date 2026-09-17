"""Small ergonomic Python façade over the generated Portable API client."""

from __future__ import annotations

import math
import os
import platform
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from importlib.resources import files
from pathlib import Path
from typing import Any

from .generated.client import *  # noqa: F401,F403
from .generated.client import (
    Client as _GeneratedClient,
    Document as _GeneratedDocument,
    FacetRequest as _GeneratedFacetRequest,
    FieldDefinition as _GeneratedFieldDefinition,
    FilterExpr as _GeneratedFilterExpr,
    InsertRequest as _GeneratedInsertRequest,
    IndexDefinition as _GeneratedIndexDefinition,
    PatchOperation as _GeneratedPatchOperation,
    PatchRequest as _GeneratedPatchRequest,
    SearchRequest as _GeneratedSearchRequest,
    SortExpr as _GeneratedSortExpr,
    Value as _GeneratedValue,
    encode_index_definition,
)
from .generated.client import __all__ as _generated_all
from .transport import NativeTransport, NativeTransportError


_UNSET = object()


def _native_library_name() -> str:
    system = platform.system()
    if system == "Linux":
        return "libsearchkernel_ffi.so"
    if system == "Darwin":
        return "libsearchkernel_ffi.dylib"
    raise NativeTransportError(f"no bundled Search Kernel library for {system}")


def _resolve_native_library() -> str:
    override = os.environ.get("SEARCHKERNEL_NATIVE_LIBRARY")
    if override:
        path = Path(override)
        if not path.is_file():
            raise NativeTransportError(
                f"SEARCHKERNEL_NATIVE_LIBRARY does not name a file: {path}"
            )
        return os.fspath(path)

    machine = platform.machine().lower()
    platform_name = {"x86_64": "linux-x86_64", "amd64": "linux-x86_64"}.get(machine)
    if platform_name is None or platform.system() != "Linux":
        raise NativeTransportError(
            "no bundled Search Kernel native library is available for "
            f"{platform.system()}-{machine}; set SEARCHKERNEL_NATIVE_LIBRARY"
        )
    resource = files("searchkernel").joinpath("native", platform_name, _native_library_name())
    if not resource.is_file():
        raise NativeTransportError(
            "Search Kernel native library is not bundled in the installed package; "
            "set SEARCHKERNEL_NATIVE_LIBRARY"
        )
    return os.fspath(resource)


def _value(value: Any) -> _GeneratedValue:
    if isinstance(value, _GeneratedValue):
        return value
    if value is None:
        return _GeneratedValue.null()
    if isinstance(value, bool):
        return _GeneratedValue.boolean(value)
    if isinstance(value, int):
        return _GeneratedValue.int64(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError("non-finite floats are not portable")
        return _GeneratedValue.float64(value)
    if isinstance(value, str):
        return _GeneratedValue.string(value)
    if isinstance(value, bytes):
        return _GeneratedValue.bytes(value)
    if isinstance(value, Mapping):
        return _GeneratedValue.object({str(key): _value(item) for key, item in value.items()})
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return _GeneratedValue.list([_value(item) for item in value])
    raise TypeError(f"unsupported portable value type: {type(value).__name__}")


def _python_value(value: _GeneratedValue) -> Any:
    if value.kind == "null":
        return None
    if value.kind in {"bool", "int64", "uint64", "float64", "string"}:
        return value.value
    if value.kind == "bytes":
        return bytes(value.value)
    if value.kind == "list":
        return [_python_value(item) for item in value.value]
    if value.kind == "object":
        return {key: _python_value(item) for key, item in value.value.items()}
    raise ValueError(f"unknown generated value kind: {value.kind!r}")


def _coerce_filter_value(value: _GeneratedValue, kind: str | None) -> _GeneratedValue:
    # Python's numeric literals are intentionally ergonomic: an integer bound
    # is valid for a floating-point field. The portable layer still receives a
    # correctly typed value after this façade-level schema lookup.
    if kind == "float64" and value.kind in {"int64", "uint64"}:
        return _GeneratedValue.float64(float(value.value))
    return value


def _coerce_filter(value: _GeneratedFilterExpr, field_kinds: dict[str, str]) -> _GeneratedFilterExpr:
    if value.kind == "comparison":
        return _GeneratedFilterExpr(
            kind=value.kind,
            field=value.field,
            op=value.op,
            value=_coerce_filter_value(value.value, field_kinds.get(value.field)),
        )
    if value.kind == "range":
        kind = field_kinds.get(value.field)
        return _GeneratedFilterExpr(
            kind=value.kind,
            field=value.field,
            lower=None if value.lower is None else _coerce_filter_value(value.lower, kind),
            upper=None if value.upper is None else _coerce_filter_value(value.upper, kind),
            include_lower=value.include_lower,
            include_upper=value.include_upper,
        )
    if value.kind in {"and", "or"}:
        return _GeneratedFilterExpr(
            kind=value.kind,
            clauses=[_coerce_filter(item, field_kinds) for item in value.clauses],
        )
    if value.kind == "not":
        return _GeneratedFilterExpr(
            kind=value.kind, clause=_coerce_filter(value.clause, field_kinds)
        )
    return value


@dataclass(frozen=True)
class _FieldSpec:
    kind: str
    stored: bool | None = None
    searchable: bool | None = None
    filterable: bool | None = None
    sortable: bool | None = None
    facetable: bool | None = None
    cardinality: str = "single"
    analyzer: Any = None

    def definition(self, name: str) -> _GeneratedFieldDefinition:
        return _GeneratedFieldDefinition(
            name=name,
            kind=self.kind,
            cardinality=self.cardinality,
            stored=self.stored,
            searchable=self.searchable,
            filterable=self.filterable,
            sortable=self.sortable,
            facetable=self.facetable,
            analyzer=self.analyzer,
        )


class Text(_FieldSpec):
    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None,
                 facetable=None, analyzer=None):
        super().__init__("text", stored, searchable, filterable, sortable, facetable, "single", analyzer)


class Keyword(_FieldSpec):
    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None,
                 facetable=None):
        super().__init__("keyword", stored, searchable, filterable, sortable, facetable)


class Integer(_FieldSpec):
    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None,
                 facetable=None):
        super().__init__("int64", stored, searchable, filterable, sortable, facetable)


class Float(_FieldSpec):
    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None,
                 facetable=None):
        super().__init__("float64", stored, searchable, filterable, sortable, facetable)


class Boolean(_FieldSpec):
    def __init__(self, *, stored=None, searchable=None, filterable=None, sortable=None,
                 facetable=None):
        super().__init__("bool", stored, searchable, filterable, sortable, facetable)


class FieldRef:
    """A field name used to construct generated filter and sort models."""

    def __init__(self, name: str):
        if not isinstance(name, str) or not name:
            raise ValueError("field name must be a non-empty string")
        self.name = name

    def _compare(self, operator: str, value: Any) -> _GeneratedFilterExpr:
        return _GeneratedFilterExpr(
            kind="comparison", field=self.name, op=operator, value=_value(value)
        )

    def __eq__(self, value: Any) -> _GeneratedFilterExpr:  # type: ignore[override]
        return self._compare("eq", value)

    def __ne__(self, value: Any) -> _GeneratedFilterExpr:  # type: ignore[override]
        return self._compare("ne", value)

    def __lt__(self, value: Any) -> _GeneratedFilterExpr:
        return self._compare("lt", value)

    def __le__(self, value: Any) -> _GeneratedFilterExpr:
        return self._compare("lte", value)

    def __gt__(self, value: Any) -> _GeneratedFilterExpr:
        return self._compare("gt", value)

    def __ge__(self, value: Any) -> _GeneratedFilterExpr:
        return self._compare("gte", value)

    def asc(self) -> _GeneratedSortExpr:
        return _GeneratedSortExpr(field=self.name, direction="asc")

    def desc(self) -> _GeneratedSortExpr:
        return _GeneratedSortExpr(field=self.name, direction="desc")


def field(name: str) -> FieldRef:
    return FieldRef(name)


class Hit(dict):
    """Dictionary-like hit with generated score and highlight metadata."""

    def __init__(self, identifier: str, fields: Mapping[str, Any], score, highlights):
        super().__init__({"_id": identifier, **fields})
        self.id = identifier
        self.score = score
        self.highlights = highlights


@dataclass
class SearchResults:
    hits: list[Hit]
    total_hits: int
    facets: dict[str, list[dict[str, Any]]]
    total_hits_relation: str


def _document(identifier: Any, values: Mapping[str, Any]) -> _GeneratedDocument:
    if not isinstance(identifier, str) or not identifier:
        raise ValueError("_id must be a non-empty string")
    if not isinstance(values, Mapping):
        raise TypeError("document must be a mapping")
    fields = {}
    for name, value in values.items():
        if not isinstance(name, str):
            raise TypeError("document field names must be strings")
        if name == "_id":
            continue
        fields[name] = _value(value)
    return _GeneratedDocument(id=identifier, fields=fields)


def _hit(value) -> Hit:
    return Hit(value.id, {key: _python_value(item) for key, item in value.fields.items()}, value.score, value.highlights)


def _field_name(value: Any, label: str = "field") -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} must be a non-empty string")
    return value


def _non_negative_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{label} must be an integer")
    if value < 0:
        raise ValueError(f"{label} must be non-negative")
    return value


def _positive_int(value: Any, label: str) -> int:
    value = _non_negative_int(value, label)
    if value == 0:
        raise ValueError(f"{label} must be positive")
    return value


class _SearchBuilderBase:
    """Shared handwritten state and ergonomics for search-like builders."""

    def __init__(self, index: "Index", query: str | None):
        self._index = index
        self._query = query
        self._filter: _GeneratedFilterExpr | None = None
        self._sort_values: list[_GeneratedSortExpr] = []
        self._facet_values: list[_GeneratedFacetRequest] = []
        self._limit: int | None = None
        self._offset: int | None = None
        self._pagination_mode: str | None = None
        self._legacy_results: SearchResults | None = None

    def filter(self, filter_expr: _GeneratedFilterExpr) -> "_SearchBuilderBase":
        if not isinstance(filter_expr, _GeneratedFilterExpr):
            raise TypeError("filter must be an existing structured FilterExpr")
        self._filter = filter_expr
        self._legacy_results = None
        return self

    def sort(self, field: str, direction: str) -> "_SearchBuilderBase":
        field = _field_name(field)
        if direction not in {"asc", "desc"}:
            raise ValueError("sort direction must be 'asc' or 'desc'")
        self._sort_values.append(_GeneratedSortExpr(field=field, direction=direction))
        self._legacy_results = None
        return self

    def facet(
        self,
        field: str,
        *,
        limit: int | None = None,
        sort: str | None = None,
    ) -> "_SearchBuilderBase":
        field = _field_name(field)
        if limit is not None:
            limit = _non_negative_int(limit, "facet limit")
        if sort is not None and sort not in {
            "count_asc",
            "count_desc",
            "value_asc",
            "value_desc",
        }:
            raise ValueError(
                "facet sort must be one of count_asc, count_desc, value_asc, value_desc"
            )
        self._facet_values.append(
            _GeneratedFacetRequest(field=field, limit=limit, sort=sort)
        )
        self._legacy_results = None
        return self

    def offset(self, value: int) -> "_SearchBuilderBase":
        self._select_offset_limit_pagination()
        self._offset = _non_negative_int(value, "offset")
        self._legacy_results = None
        return self

    def limit(self, value: int) -> "_SearchBuilderBase":
        self._select_offset_limit_pagination()
        self._limit = _non_negative_int(value, "limit")
        self._legacy_results = None
        return self

    def page(self, page_number: int, page_size: int) -> "_SearchBuilderBase":
        if self._pagination_mode is not None:
            raise ValueError("page() cannot be combined with offset() or limit()")
        page_number = _positive_int(page_number, "page number")
        page_size = _positive_int(page_size, "page size")
        self._pagination_mode = "page"
        self._offset = (page_number - 1) * page_size
        self._limit = page_size
        self._legacy_results = None
        return self

    def _select_offset_limit_pagination(self) -> None:
        if self._pagination_mode == "page":
            raise ValueError("offset() and limit() cannot be combined with page()")
        self._pagination_mode = "offset_limit"

    def _add_legacy_sort(self, values: Sequence[Any] | None) -> None:
        for value in values or []:
            if not isinstance(value, _GeneratedSortExpr):
                raise TypeError(
                    "sort values must be field(...).asc() or field(...).desc()"
                )
            if value.direction not in {"asc", "desc"}:
                raise ValueError("sort direction must be 'asc' or 'desc'")
            self._sort_values.append(value)
        self._legacy_results = None

    def _add_legacy_facets(self, values: Sequence[Any] | None) -> None:
        for value in values or []:
            if isinstance(value, str):
                self.facet(value)
            elif isinstance(value, _GeneratedFacetRequest):
                self._facet_values.append(value)
            else:
                raise TypeError(
                    "facets must contain field names or generated FacetRequest values"
                )
        self._legacy_results = None

    def _execute(self) -> SearchResults:
        self._legacy_results = self._index._execute_search(
            query=self._query,
            filter_expr=self._filter,
            sort_values=self._sort_values,
            facet_values=self._facet_values,
            limit=self._limit,
            offset=self._offset,
        )
        return self._legacy_results

    @property
    def hits(self) -> list[Hit]:
        return self._result_for_legacy_access().hits

    @property
    def total_hits(self) -> int:
        return self._result_for_legacy_access().total_hits

    @property
    def facets(self) -> dict[str, list[dict[str, Any]]]:
        return self._result_for_legacy_access().facets

    @property
    def total_hits_relation(self) -> str:
        return self._result_for_legacy_access().total_hits_relation

    def _result_for_legacy_access(self) -> SearchResults:
        if self._legacy_results is None:
            return self._execute()
        return self._legacy_results


class SearchBuilder(_SearchBuilderBase):
    """Mutable fluent builder for full-text searches."""

    def execute(self) -> SearchResults:
        return self._execute()


class BrowseBuilder(_SearchBuilderBase):
    """Mutable fluent builder for structured match-all searches."""

    def execute(self) -> SearchResults:
        return self._execute()


class PatchBuilder:
    """Mutable fluent builder for an ordered document patch."""

    def __init__(self, index: "Index", identifier: str):
        self._index = index
        self._identifier = identifier
        self._operations: list[_GeneratedPatchOperation] = []

    def set(self, field: str, value: Any) -> "PatchBuilder":
        field = _field_name(field)
        if field == "_id":
            raise ValueError("patch field must not be _id")
        self._operations.append(
            _GeneratedPatchOperation(kind="set", field=field, value=_value(value))
        )
        return self

    def unset(self, field: str) -> "PatchBuilder":
        field = _field_name(field)
        if field == "_id":
            raise ValueError("patch field must not be _id")
        self._operations.append(_GeneratedPatchOperation(kind="unset", field=field))
        return self

    def execute(self) -> None:
        self._index._client.patch(
            _GeneratedPatchRequest(id=self._identifier, operations=self._operations)
        )


class Index:
    """A synchronous on-disk Search Kernel index."""

    def __init__(self, transport: NativeTransport, field_kinds: dict[str, str] | None = None):
        self._transport = transport
        self._client = _GeneratedClient(transport)
        self._field_kinds = field_kinds or {}

    @classmethod
    def create(cls, path, *, fields: Mapping[str, _FieldSpec]) -> "Index":
        if not isinstance(fields, Mapping) or not fields:
            raise ValueError("fields must be a non-empty mapping")
        definitions = []
        field_kinds = {}
        for name, specification in fields.items():
            if not isinstance(name, str) or not name:
                raise ValueError("field names must be non-empty strings")
            if not isinstance(specification, _FieldSpec):
                raise TypeError(f"field {name!r} is not a Search Kernel field specification")
            definitions.append(specification.definition(name))
            field_kinds[name] = specification.kind
        definition = _GeneratedIndexDefinition(fields=definitions)
        transport = NativeTransport.create(
            _resolve_native_library(), path, encode_index_definition(definition)
        )
        return cls(transport, field_kinds)

    @classmethod
    def open(cls, path) -> "Index":
        transport = NativeTransport.open(_resolve_native_library(), os.fsencode(path))
        client = _GeneratedClient(transport)
        try:
            description = client.describe(DescribeRequest())
            field_kinds = {
                field.name: field.kind
                for field in (description.index.fields if description.index else [])
                if field.name and field.kind != "unspecified"
            }
        except Exception:
            try:
                transport.close()
            except Exception:
                pass
            raise
        index = cls(transport, field_kinds)
        index._client = client
        return index

    def insert(self, document: Mapping[str, Any]) -> None:
        if not isinstance(document, Mapping) or "_id" not in document:
            raise ValueError("document must contain a string _id")
        self._client.insert(
            _GeneratedInsertRequest(document=_document(document["_id"], document))
        )

    def upsert(self, document: Mapping[str, Any]) -> None:
        if not isinstance(document, Mapping) or "_id" not in document:
            raise ValueError("document must contain a string _id")
        self._client.upsert(
            UpsertRequest(document=_document(document["_id"], document))
        )

    def put(self, document: Mapping[str, Any]) -> None:
        """Compatibility alias for :meth:`upsert`."""
        self.upsert(document)

    def update(self, identifier: str, values: Mapping[str, Any]) -> None:
        if not isinstance(values, Mapping):
            raise TypeError("update fields must be a mapping")
        builder = self.patch(identifier)
        for name, value in values.items():
            if not isinstance(name, str) or not name or name == "_id":
                raise ValueError("update field names must be non-empty and not _id")
            builder.set(name, value)
        builder.execute()

    def patch(self, identifier: str) -> PatchBuilder:
        if not isinstance(identifier, str) or not identifier:
            raise ValueError("patch id must be a non-empty string")
        return PatchBuilder(self, identifier)

    def get(self, identifier: str) -> dict[str, Any] | None:
        result = self._client.get(GetRequest(id=identifier))
        if result.document is None:
            return None
        return {"_id": result.document.id, **{
            key: _python_value(value) for key, value in result.document.fields.items()
        }}

    def delete(self, identifier: str) -> bool:
        return bool(self._client.delete(DeleteRequest(id=identifier)).deleted)

    def search(
        self,
        query: str = "",
        *,
        filter=_UNSET,
        sort=_UNSET,
        facets=_UNSET,
        limit=_UNSET,
        offset=_UNSET,
    ) -> SearchBuilder:
        if not isinstance(query, str):
            raise TypeError("query must be a string")
        builder = SearchBuilder(self, query)
        legacy_call = any(
            value is not _UNSET for value in (filter, sort, facets, limit, offset)
        )
        if filter is not _UNSET and filter is not None:
            builder.filter(filter)
        if sort is not _UNSET:
            builder._add_legacy_sort(sort)
        if facets is not _UNSET:
            builder._add_legacy_facets(facets)
        if limit is not _UNSET and limit is not None:
            builder.limit(limit)
        if offset is not _UNSET and offset is not None:
            builder.offset(offset)
        if legacy_call:
            builder.execute()
        return builder

    def browse(self) -> BrowseBuilder:
        return BrowseBuilder(self, None)

    def _execute_search(
        self,
        *,
        query: str | None,
        filter_expr: _GeneratedFilterExpr | None,
        sort_values: Sequence[_GeneratedSortExpr],
        facet_values: Sequence[_GeneratedFacetRequest],
        limit: int | None,
        offset: int | None,
    ) -> SearchResults:
        if query is not None and not isinstance(query, str):
            raise TypeError("query must be a string")
        filter_value = (
            None
            if filter_expr is None
            else _coerce_filter(filter_expr, self._field_kinds)
        )
        result = self._client.search(_GeneratedSearchRequest(
            query=query,
            filter=filter_value,
            sort=list(sort_values),
            facets=list(facet_values),
            limit=limit,
            offset=offset,
        ))
        return SearchResults(
            hits=[_hit(value) for value in result.hits],
            total_hits=result.total_hits.value,
            total_hits_relation=result.total_hits.relation,
            facets={facet.field: [
                {"value": _python_value(item.value), "count": item.count}
                for item in facet.values
            ] for facet in result.facets},
        )

    def close(self) -> None:
        self._transport.close()

    def __enter__(self) -> "Index":
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()


__all__ = [
    *_generated_all,
    "Boolean",
    "BrowseBuilder",
    "FieldRef",
    "Float",
    "Hit",
    "Index",
    "Integer",
    "Keyword",
    "PatchBuilder",
    "SearchBuilder",
    "SearchResults",
    "Text",
    "field",
]
