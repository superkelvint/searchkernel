"""Canonical JSON codecs for the descriptor-generated Python models."""


def _boolean(value: Any, context: str) -> bool:
    if not isinstance(value, bool): raise ValueError(f"{context} must be a boolean")
    return value


def _value_from_json(value: dict[str, Any]) -> Value:
    kind, raw = _one_variant(value, {"null", "bool", "int64", "uint64", "float64", "string", "bytes", "list", "object"}, "value")
    if kind == "null":
        if raw is not None: raise ValueError("null value payload must be null")
        return Value.null()
    if kind == "bool":
        if not isinstance(raw, bool): raise ValueError("bool value must be a boolean")
        return Value.boolean(raw)
    if kind in ("int64", "uint64"):
        parsed = _canonical_integer(raw, unsigned=kind == "uint64")
        return Value(kind=kind, value=parsed)
    if kind == "float64":
        if isinstance(raw, bool) or not isinstance(raw, (int, float)) or not math.isfinite(raw):
            raise ValueError("float64 value must be a finite number")
        return Value.float64(raw)
    if kind == "string":
        if not isinstance(raw, str): raise ValueError("string value must be a string")
        return Value.string(raw)
    if kind == "bytes":
        if not isinstance(raw, str): raise ValueError("bytes value must be base64 text")
        return Value.bytes(base64.b64decode(raw, validate=True))
    if kind == "list":
        if not isinstance(raw, list): raise ValueError("list value must be an array")
        return Value.list([_value_from_json(x) for x in raw])
    if not isinstance(raw, dict): raise ValueError("object value must be an object")
    return Value.object({k: _value_from_json(v) for k, v in raw.items()})


def _one_variant(value: Any, allowed: set[str], context: str):
    if not isinstance(value, dict) or len(value) != 1:
        raise ValueError(f"{context} must select exactly one variant")
    kind, raw = next(iter(value.items()))
    if kind not in allowed:
        raise ValueError(f"unknown {context} variant {kind!r}")
    return kind, raw


def _fields(value: Any, allowed: set[str], context: str, required: set[str] | None = None) -> dict[str, Any]:
    if not isinstance(value, dict): raise ValueError(f"{context} must be an object")
    unknown = set(value) - allowed
    if unknown: raise ValueError(f"unknown {context} field {sorted(unknown)[0]!r}")
    missing = (required or set()) - set(value)
    if missing: raise ValueError(f"missing {context} field {sorted(missing)[0]!r}")
    return value


def _string(value: Any, context: str) -> str:
    if not isinstance(value, str): raise ValueError(f"{context} must be a string")
    return value


def _string_list(value: Any, context: str) -> list[str]:
    if not isinstance(value, list): raise ValueError(f"{context} must be an array")
    return [_string(item, f"{context}[{index}]") for index, item in enumerate(value)]


def _u32(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 2**32 - 1:
        raise ValueError(f"{context} must be an unsigned 32-bit integer")
    return value


def _enum(value: Any, allowed: set[str], context: str) -> str:
    if not isinstance(value, str) or value not in allowed:
        raise ValueError(f"unknown {context} enum value {value!r}")
    return value


def _canonical_integer(value: Any, *, unsigned: bool) -> int:
    if not isinstance(value, str): raise ValueError("64-bit integer must be a canonical string")
    if not value or (value[0] == "-" and (unsigned or len(value) == 1)):
        raise ValueError("64-bit integer is not canonical")
    digits = value[1:] if value.startswith("-") else value
    if not digits.isdigit() or (len(digits) > 1 and digits[0] == "0") or value == "-0":
        raise ValueError("64-bit integer is not canonical")
    parsed = int(value)
    if unsigned and parsed > 2**64 - 1: raise ValueError("uint64 is out of range")
    if not unsigned and not -(2**63) <= parsed <= 2**63 - 1: raise ValueError("int64 is out of range")
    return parsed


def _value_to_json(value: Value) -> dict[str, Any]:
    if value.kind == "null": return {"null": None}
    if value.kind == "bytes": return {"bytes": base64.b64encode(value.value).decode("ascii")}
    if value.kind == "list": return {"list": [_value_to_json(x) for x in value.value]}
    if value.kind == "object": return {"object": {k: _value_to_json(value.value[k]) for k in sorted(value.value)}}
    return {value.kind: value.value if value.kind not in ("int64", "uint64") else str(value.value)}


def _document_from_json(value: Optional[dict[str, Any]], result: bool = False):
    if value is None: return None
    cls = DocumentResult if result else Document
    value = _fields(value, {"id", "fields"}, "document")
    fields = value.get("fields", {})
    if not isinstance(fields, dict): raise ValueError("document fields must be an object")
    ident = _string(value["id"], "document id") if "id" in value else None
    return cls(id=ident, fields={k: _value_from_json(v) for k, v in fields.items()})


def _filter_from_json(value: Optional[dict[str, Any]]) -> Optional[FilterExpr]:
    if value is None: return None
    kind, raw = _one_variant(value, {"filter_and", "filter_or", "filter_not", "filter_exists", "filter_comparison", "filter_range"}, "filter")
    if kind in ("filter_and", "filter_or"):
        raw = _fields(raw, {"clauses"}, kind, {"clauses"})
        if not isinstance(raw["clauses"], list): raise ValueError("filter clauses must be an array")
        return FilterExpr(kind="and" if kind.endswith("and") else "or", clauses=[_filter_from_json(x) for x in raw["clauses"]])
    if kind == "filter_not":
        raw = _fields(raw, {"clause"}, kind, {"clause"})
        return FilterExpr(kind="not", clause=_filter_from_json(raw["clause"]))
    if kind == "filter_exists":
        raw = _fields(raw, {"field"}, kind, {"field"})
        return FilterExpr(kind="exists", field=_string(raw["field"], "filter field"))
    if kind == "filter_comparison":
        raw = _fields(raw, {"field", "op", "value"}, kind, {"field", "op", "value"})
        op = _string(raw["op"], "comparison operator")
        if op not in {"eq", "ne", "lt", "lte", "gt", "gte"}:
            raise ValueError(f"unknown comparison operator {op!r}")
        return FilterExpr(kind="comparison", field=_string(raw["field"], "filter field"), op=op, value=_value_from_json(raw["value"]))
    raw = _fields(raw, {"field", "lower", "upper", "include_lower", "include_upper"}, kind, {"field"})
    return FilterExpr(kind="range", field=_string(raw["field"], "range field"), lower=_value_from_json(raw["lower"]) if "lower" in raw else None, upper=_value_from_json(raw["upper"]) if "upper" in raw else None, include_lower=_boolean(raw["include_lower"], "include_lower") if "include_lower" in raw else None, include_upper=_boolean(raw["include_upper"], "include_upper") if "include_upper" in raw else None)


def _filter_to_json(value: FilterExpr) -> dict[str, Any]:
    if value.kind in ("and", "or"): return {"filter_" + value.kind: {"clauses": [_filter_to_json(x) for x in value.clauses]}}
    if value.kind == "not": return {"filter_not": {"clause": _filter_to_json(value.clause)}}
    if value.kind == "exists": return {"filter_exists": {"field": value.field}}
    if value.kind == "comparison": return {"filter_comparison": {"field": value.field, "op": value.op, "value": _value_to_json(value.value)}}
    result = {"field": value.field}
    if value.lower is not None: result["lower"] = _value_to_json(value.lower)
    if value.upper is not None: result["upper"] = _value_to_json(value.upper)
    if value.include_lower is not None: result["include_lower"] = value.include_lower
    if value.include_upper is not None: result["include_upper"] = value.include_upper
    return {"filter_range": result}


def _query_from_json(value: Optional[dict[str, Any]]) -> Optional[QueryExpr]:
    if value is None: return None
    kind, raw = _one_variant(value, {"term_query", "query_and", "query_or", "query_not"}, "query")
    if kind == "term_query":
        raw = _fields(raw, {"field", "value"}, kind, {"field", "value"})
        return QueryExpr(kind="term", field=_string(raw["field"], "query field"), value=_value_from_json(raw["value"]))
    if kind in ("query_and", "query_or"):
        raw = _fields(raw, {"clauses"}, kind, {"clauses"})
        if not isinstance(raw["clauses"], list): raise ValueError("query clauses must be an array")
        return QueryExpr(kind="and" if kind.endswith("and") else "or", clauses=[_query_from_json(x) for x in raw["clauses"]])
    raw = _fields(raw, {"clause"}, kind, {"clause"})
    return QueryExpr(kind="not", clause=_query_from_json(raw["clause"]))


def _query_to_json(value: QueryExpr) -> dict[str, Any]:
    if value.kind == "term": return {"term_query": {"field": value.field, "value": _value_to_json(value.value)}}
    if value.kind in ("and", "or"): return {"query_" + value.kind: {"clauses": [_query_to_json(x) for x in value.clauses]}}
    return {"query_not": {"clause": _query_to_json(value.clause)}}


def _search_from_json(value: dict[str, Any]) -> SearchRequest:
    value = _fields(value, {"query", "search_fields", "filter", "sort", "facets", "ranking", "limit", "offset", "projection", "highlighting", "language", "query_expr", "timeout"}, "search")
    if "query" in value: query = _string(value["query"], "search.query")
    else: query = None
    if "search_fields" in value:
        selection = value["search_fields"]
        if not isinstance(selection, list): raise ValueError("search.search_fields must be an array")
        search_fields = SearchFieldSelection(fields=[])
        for index, item in enumerate(selection):
            item = _fields(item, {"field", "weight"}, f"search field {index}", {"field"})
            field = _string(item["field"], f"search field {index}.field")
            weight = item.get("weight")
            if "weight" in item and (isinstance(weight, bool) or not isinstance(weight, (int, float)) or not math.isfinite(weight)):
                raise ValueError(f"search field {index}.weight must be a finite number")
            search_fields.fields.append(SearchField(field=field, weight=weight))
    else:
        search_fields = None
    projection = None if "projection" not in value else FieldProjection(fields=_string_list(value["projection"], "search.projection"))
    highlighting = None if "highlighting" not in value else HighlightRequest(fields=_string_list(value["highlighting"], "search.highlighting"))
    if "filter" in value and value["filter"] is None: raise ValueError("search.filter does not accept JSON null")
    filter_value = _filter_from_json(value["filter"]) if "filter" in value else None
    if "sort" in value:
        raw_sort = value["sort"]
        if not isinstance(raw_sort, list): raise ValueError("search.sort must be an array")
    else:
        raw_sort = []
    sort = []
    for index, item in enumerate(raw_sort):
        item = _fields(item, {"field", "direction", "missing"}, f"sort {index}", {"field"})
        field = _string(item["field"], f"sort {index}.field")
        direction = _enum(item.get("direction", "unspecified"), {"unspecified", "asc", "desc"}, f"sort {index}.direction")
        missing = None if "missing" not in item else _enum(item["missing"], {"unspecified", "first", "last"}, f"sort {index}.missing")
        sort.append(SortExpr(field=field, direction=direction, missing=missing))
    if "facets" in value:
        raw_facets = value["facets"]
        if not isinstance(raw_facets, list): raise ValueError("search.facets must be an array")
    else:
        raw_facets = []
    facets = []
    for index, item in enumerate(raw_facets):
        item = _fields(item, {"field", "limit", "sort"}, f"facet {index}", {"field"})
        field = _string(item["field"], f"facet {index}.field")
        limit = None if "limit" not in item else _u32(item["limit"], f"facet {index}.limit")
        facet_sort = None if "sort" not in item else _enum(item["sort"], {"unspecified", "count_desc", "count_asc", "value_asc", "value_desc"}, f"facet {index}.sort")
        facets.append(FacetRequest(field=field, limit=limit, sort=facet_sort))
    if "ranking" not in value:
        ranking = None
    else:
        ranking_kind, ranking_payload = _one_variant(value["ranking"], {"default_relevance_v1", "bm25_ranking"}, "ranking")
        _fields(ranking_payload, set(), "ranking payload")
        ranking = RankingPolicy(kind=ranking_kind)
    limit = None if "limit" not in value else _u32(value["limit"], "search.limit")
    offset = None if "offset" not in value else _u32(value["offset"], "search.offset")
    language = None if "language" not in value else _string(value["language"], "search.language")
    if "query_expr" in value and value["query_expr"] is None: raise ValueError("search.query_expr does not accept JSON null")
    query_expr = _query_from_json(value["query_expr"]) if "query_expr" in value else None
    timeout = None if "timeout" not in value else _canonical_integer(value["timeout"], unsigned=True)
    return SearchRequest(query=query, search_fields=search_fields, filter=filter_value, sort=sort, facets=facets, ranking=ranking, limit=limit, offset=offset, projection=projection, highlighting=highlighting, language=language, query_expr=query_expr, timeout=timeout)


def _search_to_json(value: SearchRequest) -> dict[str, Any]:
    result = {}
    if value.query is not None: result["query"] = value.query
    if value.search_fields is not None: result["search_fields"] = [{"field": x.field, **({"weight": x.weight} if x.weight is not None else {})} for x in value.search_fields.fields]
    if value.filter is not None: result["filter"] = _filter_to_json(value.filter)
    if value.sort: result["sort"] = [{"field": x.field, "direction": x.direction, **({"missing": x.missing} if x.missing is not None else {})} for x in value.sort]
    if value.facets: result["facets"] = [{"field": x.field, **({"limit": x.limit} if x.limit is not None else {}), **({"sort": x.sort} if x.sort is not None else {})} for x in value.facets]
    if value.ranking is not None: result["ranking"] = {value.ranking.kind: {}}
    if value.limit is not None: result["limit"] = value.limit
    if value.offset is not None: result["offset"] = value.offset
    if value.projection is not None: result["projection"] = value.projection.fields
    if value.highlighting is not None: result["highlighting"] = value.highlighting.fields
    if value.language is not None: result["language"] = value.language
    if value.query_expr is not None: result["query_expr"] = _query_to_json(value.query_expr)
    if value.timeout is not None: result["timeout"] = str(value.timeout)
    return result


def request_from_json(value: dict[str, Any]):
    """Convert an already-decoded request object to generated models.

    Use request_from_json_text for canonical JSON text and duplicate-key rejection.
    """
    value = _fields(value, {"protocol_version", "request"}, "request envelope", {"protocol_version", "request"})
    if isinstance(value["protocol_version"], bool) or value["protocol_version"] != 1: raise ValueError("unsupported request protocol version")
    operation, payload = _one_variant(value["request"], set(_OPERATIONS), "request")
    if operation in ("insert", "upsert"):
        payload = _fields(payload, {"document"}, operation, {"document"})
        cls = InsertRequest if operation == "insert" else UpsertRequest; return operation, cls(document=_document_from_json(payload["document"]))
    if operation == "patch":
        payload = _fields(payload, {"id", "operations"}, operation, {"id"})
        operations = []
        raw_operations = payload.get("operations", [])
        if not isinstance(raw_operations, list): raise ValueError("patch operations must be an array")
        for item in raw_operations:
            kind, raw = _one_variant(item, {"none", "set_field", "unset_field"}, "patch operation")
            if kind == "none":
                _fields(raw, set(), kind)
                operations.append(PatchOperation(kind="none"))
                continue
            raw = _fields(raw, {"field", "value"} if kind == "set_field" else {"field"}, kind, {"field"} | ({"value"} if kind == "set_field" else set()))
            operations.append(PatchOperation(kind={"set_field": "set", "unset_field": "unset"}[kind], field=_string(raw["field"], "patch field"), value=_value_from_json(raw["value"]) if kind == "set_field" else None))
        return operation, PatchRequest(id=_string(payload["id"], "patch id"), operations=operations)
    if operation == "delete":
        payload = _fields(payload, {"id"}, operation, {"id"}); return operation, DeleteRequest(id=_string(payload["id"], "delete id"))
    if operation == "get":
        payload = _fields(payload, {"id", "projection"}, operation, {"id"}); return operation, GetRequest(id=_string(payload["id"], "get id"), projection=FieldProjection(fields=_string_list(payload["projection"], "get.projection")) if "projection" in payload else None)
    if operation == "search": return operation, _search_from_json(payload)
    if operation == "analyze":
        payload = _fields(payload, {"target", "text", "language"}, operation, {"text"})
        raw_target = payload.get("target")
        if raw_target is None:
            target = None
        else:
            kind, raw = _one_variant(raw_target, {"analyzer", "field", "none"}, "analyze target")
            if kind == "analyzer":
                raw = _fields(raw, {"name", "version", "options"}, kind, {"name"})
                options = raw.get("options", {})
                if not isinstance(options, dict): raise ValueError("analyzer options must be an object")
                target = AnalyzeTarget(kind="analyzer", value=Analyzer(name=_string(raw["name"], "analyzer name"), version=_u32(raw.get("version", 0), "analyzer version"), options={k: _value_from_json(v) for k, v in options.items()}))
            elif kind == "field":
                raw = _fields(raw, {"field"}, kind, {"field"}); target = AnalyzeTarget(kind="field", value=_string(raw["field"], "analyze field"))
            else:
                target = AnalyzeTarget(kind="none")
        return operation, AnalyzeRequest(target=target, text=_string(payload["text"], "analyze text"), language=_string(payload["language"], "analyze language") if "language" in payload else None)
    if operation == "describe":
        _fields(payload, set(), operation); return operation, DescribeRequest()
    if operation == "inspect_query":
        payload = _fields(payload, {"request"}, operation); return operation, InspectQueryRequest(request=_search_from_json(payload["request"]) if "request" in payload else None)
    raise ValueError(f"unknown request operation {operation!r}")


def _reject_duplicate_keys(pairs):
    value = {}
    for key, item in pairs:
        if key in value: raise ValueError(f"duplicate object field {key!r}")
        value[key] = item
    return value


def _reject_non_json_constant(value):
    raise ValueError(f"non-canonical JSON constant {value!r}")


def request_from_json_text(text: str):
    """Construct a request from canonical JSON text, rejecting duplicate keys."""
    if not isinstance(text, str): raise TypeError("request JSON must be text")
    return request_from_json(json.loads(text, object_pairs_hook=_reject_duplicate_keys, parse_constant=_reject_non_json_constant))


def _response_to_json(operation: str, value: Any) -> dict[str, Any]:
    if operation in ("insert", "upsert", "patch"): return {operation: {"id": value.id}}
    if operation == "delete": return {"delete": {"id": value.id, "deleted": value.deleted}}
    if operation == "get":
        payload = {}
        if value.document is not None:
            payload["document"] = {"id": value.document.id, "fields": {k: _value_to_json(v) for k, v in sorted(value.document.fields.items())}}
        return {"get": payload}
    if operation == "analyze": return {"analyze": {"tokens": [{"surface": x.surface, "term": x.term, "position": x.position, "start_byte": x.start_byte, "end_byte": x.end_byte} for x in value.tokens]}}
    if operation == "describe":
        if value.index is None:
            index = None
        else:
            index = {}
            if value.index.name is not None:
                index["name"] = value.index.name
            if value.index.fields:
                index["fields"] = [
                    {
                        "name": x.name,
                        "kind": x.kind,
                        "cardinality": x.cardinality,
                        "stored": x.stored,
                        "searchable": x.searchable,
                        "filterable": x.filterable,
                        "sortable": x.sortable,
                        "facetable": x.facetable,
                    }
                    for x in value.index.fields
                ]
        return {"describe": {"index": index}}
    if operation == "inspect_query": return {"inspect_query": {"resolved_request": _search_to_json(value.resolved_request) if value.resolved_request else None}}
    hits = []
    for hit in value.hits:
        highlights = []
        for highlight in hit.highlights:
            fragments = []
            for fragment in highlight.fragments:
                fragments.append({
                    "text": fragment.text,
                    "matches": [{"start_byte": span.start_byte, "end_byte": span.end_byte}
                                for span in fragment.matches],
                })
            highlights.append({"field": highlight.field, "fragments": fragments})
        hits.append({
            "id": hit.id,
            "score": hit.score,
            "fields": {key: _value_to_json(item) for key, item in sorted(hit.fields.items())},
            "highlights": highlights,
        })
    facets = []
    for facet in value.facets:
        facets.append({
            "field": facet.field,
            "values": [{"value": _value_to_json(item.value), "count": str(item.count)}
                       for item in facet.values],
        })
    return {"search": {"hits": hits, "total_hits": {
        "value": str(value.total_hits.value), "relation": value.total_hits.relation,
    }, "facets": facets}}
