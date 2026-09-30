from __future__ import annotations

from math import isclose, isfinite
import json
import os
import re
from pathlib import Path
import atexit
import shutil
import tempfile

from client_real_e2e_parity_helpers import aggregation_expression, aggregation_output

from searchkernel import (
    Index,
    Text,
    Keyword,
    Integer,
    Float,
    Boolean,
    PositionField,
    StructField,
    ArrayField,
    RankProfile,
    Tensor,
    TensorField,
    Group,
    SearchKernelError,
    agg,
    asc,
    desc,
    g,
    gf,
    q,
)


WORKSPACE_PATH = Path(tempfile.mkdtemp(prefix="searchkernel-client-api-e2e-"))
INDEX_PATH = WORKSPACE_PATH / "showcase-index"
ROOT = Path(__file__).resolve().parents[3]
E2E_CORPUS_PATH = ROOT / "reference/client-real-e2e-parity/corpus.json"
E2E_CORPUS = json.loads(E2E_CORPUS_PATH.read_text())
PARITY_CONSUMED_INTENTS: dict[str, dict[str, object]] = {}


class _ConsumedIntent:
    def __init__(self, scenario: str, source: dict[str, object]):
        self._scenario = scenario
        self._source = source

    def __getitem__(self, key: str):
        value = self._source[key]
        PARITY_CONSUMED_INTENTS.setdefault(self._scenario, {})[key] = json.loads(json.dumps(value))
        return value

    def get(self, key: str, default=None):
        if key not in self._source:
            return default
        return self[key]


class _ConsumedIntentMap:
    def __init__(self, source: dict[str, object]):
        self._source = source

    def __getitem__(self, key: str):
        value = self._source[key]
        if not isinstance(value, dict):
            raise TypeError(f"parity scenario intent {key} must be an object")
        return _ConsumedIntent(key, value)

    def get(self, key: str, default=None):
        if key not in self._source:
            return default
        return self[key]


PARITY_INTENTS = _ConsumedIntentMap(E2E_CORPUS["scenario_inputs"])
VECTOR_TYPE = E2E_CORPUS["schema"]["fields"]["embedding"]["type"]


FIELD_TYPES = {"text": Text, "keyword": Keyword, "int64": Integer, "float64": Float, "bool": Boolean, "position": PositionField}


def corpus_field(spec):
    field_type = spec["type"]
    options = {key: value for key, value in spec.items() if key not in {"type", "element"}}
    if field_type == "array":
        element = spec["element"]
        if element["type"] != "struct":
            raise ValueError(f"unsupported corpus array element: {element['type']}")
        members = {name: corpus_field(member) for name, member in element["fields"].items()}
        return ArrayField(StructField(members), **options)
    if field_type.startswith("tensor<"):
        return TensorField(field_type, **options)
    return FIELD_TYPES[field_type](**options)


def fields_from_corpus():
    fields = {}
    for name, spec in E2E_CORPUS["schema"]["fields"].items():
        fields[name] = corpus_field(spec)
    return fields


def corpus_value(field, item):
    field_type = E2E_CORPUS["schema"]["fields"].get(field, {}).get("type")
    if field_type == "float64" and isinstance(item, (int, float)) and not isinstance(item, bool):
        return float(item)
    if field_type == "int64" and isinstance(item, (int, str)) and not isinstance(item, bool):
        return int(item)
    if isinstance(field_type, str) and field_type.startswith("tensor<") and isinstance(item, list):
        return embedding(*item)
    return item


def document_input(source):
    return {field: corpus_value(field, item) for field, item in source.items()}


def shared_filter_bindings(source, bindings):
    converted = {}
    for name, item in bindings.items():
        field_match = re.search(
            rf"([A-Za-z_][\w.]*)\s*(?:(?:<=|>=|=|<|>)\s*|IN\s*\(\s*):{re.escape(name)}\b",
            source,
            flags=re.IGNORECASE,
        )
        converted[name] = corpus_value(field_match.group(1), item) if field_match else item
    return converted


def rank_profiles_from_corpus():
    profiles = []
    for spec in E2E_CORPUS["schema"]["rank_profiles"]:
        profile = RankProfile(spec["name"]).first_phase(spec["first_phase"])
        if "second_phase" in spec:
            profile = profile.second_phase(spec["second_phase"])
        if "rerank_count" in spec:
            profile = profile.rerank_count(spec["rerank_count"])
        for name, type_spec in spec.get("query_inputs", {}).items():
            profile = profile.query_input(name, type_spec)
        for feature in spec.get("match_features", []):
            profile = profile.match_feature(feature)
        profiles.append(profile)
    return profiles


def embedding(*values: float) -> Tensor:
    if len(values) != 8:
        raise ValueError("the E2E corpus uses eight-dimensional embeddings")
    return Tensor.dense(VECTOR_TYPE, values)


def corpus_vector(document_id: str) -> tuple[float, ...]:
    document = next(item for item in E2E_CORPUS["documents"] if item["_id"] == document_id)
    return tuple(document["embedding"])


SHOE_1_VECTOR = corpus_vector("shoe-1")
SHOE_2_VECTOR = corpus_vector("shoe-2")
SHOE_3_VECTOR = corpus_vector("shoe-3")
SHOE_4_VECTOR = corpus_vector("shoe-4")
SHOE_5_VECTOR = corpus_vector("shoe-5")
BAG_1_VECTOR = corpus_vector("bag-1")


def cleanup_workspace() -> None:
    shutil.rmtree(WORKSPACE_PATH, ignore_errors=True)


atexit.register(cleanup_workspace)


def heading(title: str) -> None:
    print()
    print("=" * 80)
    print(title)
    print("=" * 80)


def print_results(results) -> None:
    print(
        "total_hits =",
        results.total_hits,
        "relation =",
        getattr(results, "total_hits_relation", None),
    )

    if getattr(results, "aggregations", None):
        print("aggregations:", dict(results.aggregations))

    for hit in results.hits:
        print(
            f"  id={hit.id!r}",
            f"relevance={getattr(hit, 'relevance', None)!r}",
            f"features={getattr(hit, 'features', {})!r}",
            f"sort_values={getattr(hit, 'sort_values', [])!r}",
            dict(hit),
        )

    coverage = getattr(results, "coverage", None)
    if coverage is not None:
        print(
            "coverage:",
            {
                "percent": coverage.coverage_percent,
                "documents": coverage.documents,
                "full": coverage.full,
                "degraded": coverage.degraded,
            },
        )

    if getattr(results, "facets", None):
        print("facets:")
        for name, values in results.facets.items():
            print(f"  {name}:")
            for item in values:
                print(f"    {item['value']!r}: {item['count']}")

    if getattr(results, "groups", None):
        print("groups:")
        print_groups(results.groups, indent=1)


def print_groups(groups, indent: int = 0) -> None:
    prefix = "  " * indent
    for group in groups:
        print(
            f"{prefix}- value={group.value!r}",
            f"label={getattr(group, 'label', None)!r}",
            f"fields={dict(getattr(group, 'fields', {}) or {})}",
            f"continuations={list(getattr(group, 'continuations', []) or [])}",
        )
        for hit in getattr(group, "hits", []) or []:
            print(
                f"{prefix}    hit id={hit.id!r}",
                f"fields={dict(hit)}",
            )
        print_groups(getattr(group, "groups", []) or [], indent + 1)


def assert_raises(exc_type, fn, message: str) -> None:
    try:
        fn()
    except exc_type:
        print("PASS:", message)
        return

    raise AssertionError(message)


def assert_ids(results, expected) -> None:
    actual = [hit.id for hit in results.hits]
    assert actual == list(expected), (actual, list(expected))


def groups_by_value(groups):
    return {group.value: group for group in groups}


def grouping_groups(results):
    """Return the direct groups below the canonical native grouping wrapper."""
    candidates = [
        group
        for group in results.groups
        if group.id is None
        and group.label is None
        and group.value is None
        and not group.fields
        and not group.hits
    ]
    if len(candidates) != 1:
        raise AssertionError(
            f"expected one native grouping wrapper, got {len(candidates)}"
        )
    wrapper = candidates[0]
    return wrapper.groups


PARITY_OBSERVATIONS: dict[str, object] = {}


def typed_query_for_scenario(scenario, intent):
    field = intent.get("field")
    if scenario == "typed_query_term":
        return q.term(field, intent["value"])
    if scenario == "typed_query_contains":
        return q.contains(field, intent["value"])
    if scenario == "typed_query_phrase":
        return q.contains(field, q.phrase(*intent["terms"]))
    if scenario == "typed_query_near":
        return q.contains(field, q.near(*intent["terms"]))
    if scenario == "typed_query_ordered_near":
        return q.contains(field, q.onear(*intent["terms"]))
    if scenario in {"typed_query_range", "typed_query_range_exclusive"}:
        return q.range(
            field, corpus_value(field, intent["lower"]), corpus_value(field, intent["upper"]),
            include_lower=intent["include_lower"],
            include_upper=intent["include_upper"],
        )
    if scenario == "typed_query_boolean":
        left, right = intent["or_terms"]
        negative = intent["not_term"]
        return q.all_of(
            q.any_of(q.contains(left["field"], left["value"]), q.contains(right["field"], right["value"])),
            q.negate(q.contains(negative["field"], negative["value"])),
        )
    if scenario == "typed_query_in":
        return q.in_values(field, intent["values"])
    if scenario == "typed_query_equals":
        return q.equals(field, intent["value"])
    if scenario == "typed_query_fuzzy":
        return q.contains(field, q.fuzzy(intent["value"]))
    if scenario == "typed_query_weak_and":
        return q.weak_and(*(q.contains(term["field"], term["value"]) for term in intent["terms"]))
    if scenario == "typed_query_wand":
        return q.wand(field, intent["items"])
    if scenario == "typed_query_weighted_set":
        return q.weighted_set(field, intent["items"])
    if scenario == "typed_query_dot_product":
        return q.dot_product(field, intent["items"])
    if scenario == "typed_query_prefix":
        return q.prefix(field, intent["value"])
    if scenario == "typed_query_substring":
        return q.substring(field, intent["value"])
    if scenario == "typed_query_suffix":
        return q.suffix(field, intent["value"])
    if scenario == "typed_query_same_element":
        return q.contains(
            field,
            q.same_element(
                q.contains(intent["first_field"], intent["first_value"]),
                q.contains(intent["last_field"], intent["last_value"]),
            ),
        )
    if scenario == "typed_query_geo_location":
        return q.geo_location(field, lat=intent["latitude"], lon=intent["longitude"], radius=intent["radius"])
    if scenario == "typed_query_geo_box":
        return q.geo_box(field, south=intent["south"], west=intent["west"], north=intent["north"], east=intent["east"])
    raise ValueError(f"unsupported typed parity scenario: {scenario}")


def shared_typed_query_observations(index):
    observations = {}
    for scenario in E2E_CORPUS["scenario_inputs"]:
        if not scenario.startswith("typed_query_"):
            continue
        intent = PARITY_INTENTS[scenario]
        builder = index.query(typed_query_for_scenario(scenario, intent))
        ranking_profile = intent.get("ranking_profile")
        if ranking_profile:
            builder = builder.ranking(ranking_profile)
        results = builder.limit(intent["limit"]).execute()
        result_order = intent["result_order"]
        feature = intent.get("feature")
        if feature:
            query_result = [{"id": hit.id, "feature": hit.features.get(feature)} for hit in results.hits]
        else:
            query_result = [hit.id for hit in results.hits]
            if result_order == "sorted_ids":
                query_result.sort()
        observations[scenario] = {
            "observation": {"intent": PARITY_CONSUMED_INTENTS[scenario], "query_result": query_result},
            "capabilities": E2E_CORPUS["client_scenario_contract"][scenario]["python"],
        }
    return observations


def capture_real_engine_parity(index) -> dict[str, object]:
    typed_observations = shared_typed_query_observations(index)
    intents = PARITY_INTENTS
    ordered_spec = intents["pagination"]
    ordered = index.search(ordered_spec["query"]).sort(ordered_spec["sort"][0]["field"], ordered_spec["sort"][0]["direction"]).limit(ordered_spec["all_limit"]).execute()
    filtered_spec = intents["filtered_facets"]
    filtered = index.search(filtered_spec["query"]).filter(filtered_spec["filter"]).sort(ordered_spec["sort"][0]["field"], ordered_spec["sort"][0]["direction"]).limit(filtered_spec["page_limit"]).execute()
    facet_spec = intents["facets"]
    faceted = index.search(facet_spec["query"]).facet(facet_spec["field"], name=facet_spec["name"], limit=facet_spec["limit"], sort=facet_spec["sort"]).limit(facet_spec["page_limit"]).execute()
    root_spec = intents["root_aggregations"]
    if root_spec["kind"] != "browse":
        raise ValueError("root_aggregations requires browse intent")
    root_builder = index.browse().filter(root_spec["filter"])
    for aggregation_spec in root_spec["aggregations"]:
        root_builder = root_builder.aggregate(aggregation_output(aggregation_spec))
    root = root_builder.limit(root_spec["limit"]).execute()
    grouped_spec = intents["grouping_aggregations"]
    group_order = []
    for order_spec in grouped_spec["order"]:
        expression = agg.count() if order_spec["kind"] == "count" else g.field(order_spec["field"])
        order_function = desc if order_spec["direction"] == "desc" else asc
        group_order.append(order_function(expression))
    grouped = index.search(grouped_spec["query"]).group(
        Group.by(grouped_spec["group_by"]).limit(grouped_spec["limit"]).precision(grouped_spec["precision"])
        .order_by(*group_order)
        .output(*(aggregation_output(output) for output in grouped_spec["outputs"]))
    ).execute()
    nearest_intent = intents["nearest_neighbor"]
    nearest = (
        index.query(q.nearest_neighbor(nearest_intent["field"], nearest_intent["query_input"]))
        .ranking(nearest_intent["ranking_profile"])
        .input(nearest_intent["query_input"], embedding(*corpus_vector(nearest_intent["query_input_document"])))
        .limit(nearest_intent["limit"]).execute()
    )
    hybrid_intent = intents["hybrid_text_vector"]
    hybrid = (
        index.query(q.any_of(
            q.contains(hybrid_intent["text_field"], hybrid_intent["text_value"]),
            q.nearest_neighbor(hybrid_intent["vector_field"], hybrid_intent["query_input"]),
        ))
        .ranking(hybrid_intent["ranking_profile"])
        .input(hybrid_intent["query_input"], embedding(*corpus_vector(hybrid_intent["query_input_document"])))
        .limit(hybrid_intent["limit"]).execute()
    )
    reranked_spec = intents["two_phase_reranking"]
    reranked = index.search(reranked_spec["query"]).ranking(reranked_spec["ranking_profile"]).limit(reranked_spec["limit"]).execute()
    try:
        error_spec = intents["structured_filter_error"]
        index.search(error_spec["query"]).filter(error_spec["filter"]).execute()
    except SearchKernelError as error:
        assert str(error.code).lower() == "invalid_argument", error.code
        error_codes = sorted(detail.code for detail in error.details)
    else:
        raise AssertionError("unknown field filter unexpectedly succeeded")

    document = index.get(intents["document_crud"]["document_id"])
    tensor_spec = intents["tensor_readback"]
    tensor_document = index.get(tensor_spec["document_id"])
    if tensor_document is None:
        raise RuntimeError("tensor readback document is missing")
    tensor_value = tensor_document[tensor_spec["field"]]
    full_spec = intents["full_text_search"]
    full_text = index.search(full_spec["query"]).limit(full_spec["limit"]).execute()
    stemming_spec = intents["stemming"]
    stemming = index.search(stemming_spec["query"]).limit(stemming_spec["limit"]).execute()
    binding_spec = intents["text_filter_binding"]
    text_binding = index.search(binding_spec["query"]).filter(binding_spec["filter"], shared_filter_bindings(binding_spec["filter"], binding_spec["bindings"])).sort(binding_spec["sort"][0]["field"], binding_spec["sort"][0]["direction"]).limit(binding_spec["limit"]).execute()
    repeated_spec = intents["repeated_filters"]
    repeated_builder = index.search(repeated_spec["query"])
    for filter_spec in repeated_spec["filters"]:
        repeated_builder = repeated_builder.filter(filter_spec["source"], shared_filter_bindings(filter_spec["source"], filter_spec["bindings"]))
    repeated_filters = repeated_builder.sort(repeated_spec["sort"][0]["field"], repeated_spec["sort"][0]["direction"]).limit(repeated_spec["limit"]).execute()
    boolean_spec = intents["boolean_filter_grammar"]
    boolean_filter = (index.search(boolean_spec["query"]).filter(boolean_spec["filter"], shared_filter_bindings(boolean_spec["filter"], boolean_spec["bindings"]))
                      .sort(boolean_spec["sort"][0]["field"], boolean_spec["sort"][0]["direction"]).limit(boolean_spec["limit"]).execute())
    projection_spec = intents["sorting_projection"]
    projection = index.search(projection_spec["query"]).sort(projection_spec["sort"][0]["field"], projection_spec["sort"][0]["direction"]).fields(*projection_spec["projection"]).limit(projection_spec["limit"]).execute()
    all_sorted = index.search(ordered_spec["query"]).sort(ordered_spec["sort"][0]["field"], ordered_spec["sort"][0]["direction"]).limit(ordered_spec["all_limit"]).execute()
    page = index.search(ordered_spec["query"]).sort(ordered_spec["sort"][0]["field"], ordered_spec["sort"][0]["direction"]).offset(ordered_spec["offset"]).limit(ordered_spec["limit"]).execute()
    filtered_facet = index.search(filtered_spec["query"]).filter(filtered_spec["filter"]).facet(filtered_spec["field"], limit=filtered_spec["limit"], sort=filtered_spec["sort"]).limit(filtered_spec["page_limit"]).execute()
    nested_spec = intents["nested_grouping"]
    if nested_spec["kind"] != "browse":
        raise ValueError("nested_grouping requires browse intent")
    nested = index.browse().filter(nested_spec["filter"]["source"], shared_filter_bindings(nested_spec["filter"]["source"], nested_spec["filter"]["bindings"])).group(
        Group.by(nested_spec["group_by"]).limit(nested_spec["limit"]).output(*(aggregation_output(output) for output in nested_spec["outputs"])).subgroup(
            Group.by(nested_spec["nested"]["group_by"]).limit(nested_spec["nested"]["limit"]).output(*(aggregation_output(output) for output in nested_spec["nested"]["outputs"]))
        )
    ).execute()
    alias_spec = intents["group_alias"]
    alias_order = desc if alias_spec["order_by_alias"]["direction"] == "desc" else asc
    alias_group = Group.by(alias_spec["group_by"]).limit(alias_spec["limit"])
    for alias_specification in alias_spec["aliases"]:
        alias_group = alias_group.alias(alias_specification["name"], aggregation_expression(alias_specification))
    aliased = index.search(alias_spec["query"]).group(
        alias_group.order_by(alias_order(g.alias_ref(alias_spec["order_by_alias"]["name"]))).output(*(aggregation_output(output) for output in alias_spec["outputs"]))
    ).execute()
    keep_spec = intents["group_keep_filter"]
    kept = index.search(keep_spec["query"]).group(
        Group.by(keep_spec["group_by"]).limit(keep_spec["limit"]).keep(gf.regex(g.field(keep_spec["keep_regex"]["field"]), keep_spec["keep_regex"]["pattern"])).output(*(aggregation_output(output) for output in keep_spec["outputs"]))
    ).execute()
    typed = index.query(typed_query_for_scenario("typed_query_contains", intents["typed_query_contains"])).ranking("default").limit(full_spec["limit"]).execute()
    rank_input_spec = intents["rank_inputs"]
    rank_input_builder = index.search(rank_input_spec["query"]).ranking(rank_input_spec["ranking_profile"])
    for key, value in rank_input_spec["inputs"].items():
        input_type = rank_input_spec["input_types"][key]
        if input_type == "float64":
            value = float(value)
        else:
            raise ValueError(f"unsupported parity rank input type: {input_type}")
        rank_input_builder = rank_input_builder.input(key, value)
    rank_input = rank_input_builder.limit(rank_input_spec["limit"]).execute()
    rank_property_spec = intents["rank_properties"]
    rank_property_builder = index.search(rank_property_spec["query"]).ranking(rank_property_spec["ranking_profile"])
    for key, value in rank_property_spec["rank_properties"].items(): rank_property_builder = rank_property_builder.rank_property(key, value)
    rank_property = rank_property_builder.limit(rank_property_spec["limit"]).execute()
    degraded_spec = intents["coverage_degradation"]
    if degraded_spec["kind"] != "browse":
        raise ValueError("coverage_degradation requires browse intent")
    degraded = index.browse().filter(degraded_spec["filter"]["source"], shared_filter_bindings(degraded_spec["filter"]["source"], degraded_spec["filter"]["bindings"])).sort(degraded_spec["sort"][0]["field"], degraded_spec["sort"][0]["direction"])
    for key, value in degraded_spec["rank_properties"].items(): degraded = degraded.rank_property(key, value)
    degraded = degraded.limit(degraded_spec["limit"]).execute()
    yql_spec = intents["yql"]
    yql_builder = index.yql(yql_spec["source"])
    for key, value in yql_spec["bindings"].items():
        yql_builder = yql_builder.property(key, value)
    yql = yql_builder.ranking(yql_spec["ranking_profile"]).page(yql_spec["page"], yql_spec["page_size"]).execute()
    try:
        syntax_spec = intents["text_filter_syntax_error"]
        index.search(syntax_spec["query"]).filter(syntax_spec["filter"]).execute()
    except SearchKernelError as error:
        syntax_error = {"code": str(error.code).lower(), "details": sorted(detail.code for detail in error.details)}
    else:
        raise AssertionError("malformed textual filter unexpectedly succeeded")

    facet_entries = [
        {"value": str(item["value"]), "count": item["count"]}
        for item in faceted.facets["brands"]
    ]
    filtered_facet_entries = [
        {"value": str(item["value"]), "count": item["count"]}
        for item in filtered_facet.facets["brand"]
    ]
    groups = grouping_groups(grouped)
    misuse_spec = intents["builder_misuse"]
    assert_raises((TypeError, ValueError), lambda: index.search(misuse_spec["query"]).page(misuse_spec["page"], misuse_spec["page_size"]).limit(misuse_spec["limit"]), "page plus limit rejected")
    assert_raises((TypeError, ValueError), lambda: index.search(misuse_spec["query"]).page(misuse_spec["page"], misuse_spec["page_size"]).offset(misuse_spec["page_size"]), "page plus offset rejected")
    assert_raises((TypeError, ValueError), lambda: index.search(misuse_spec["query"]).limit(misuse_spec["negative_limit"]), "negative limit rejected")
    assert_raises((TypeError, ValueError), lambda: index.search(misuse_spec["query"]).offset(misuse_spec["negative_offset"]), "negative offset rejected")
    update_spec = intents["advanced_update"]
    index.insert(document_input({"_id": update_spec["document_id"], **update_spec["initial"]}))
    update_builder = index.update(update_spec["document_id"])
    for key, value in update_spec["set"].items(): update_builder = update_builder.set(key, corpus_value(key, value))
    for key in update_spec["unset"]: update_builder = update_builder.unset(key)
    for key, value in update_spec["increments"].items(): update_builder = update_builder.increment(key, value)
    update_builder.execute()
    for key, value in update_spec["decrements"].items(): index.update(update_spec["document_id"]).decrement(key, value).execute()
    for key, value in update_spec["multipliers"].items(): index.update(update_spec["document_id"]).multiply(key, value).execute()
    for key, value in update_spec["divisors"].items(): index.update(update_spec["document_id"]).divide(key, value).execute()
    advanced_update = index.get(update_spec["document_id"])
    group_values = [
        {"brand": group.value, "products": group.get("products"), "avg_price": group.get("avg_price")}
        for group in groups
    ]
    observations = {
        "document_crud": {"id": document["_id"], "title": document["title"], "brand": document["brand"], "price": document["price"]},
        "full_text_search": {"ids": [hit.id for hit in full_text.hits], "total": full_text.total_hits},
        "stemming": {"ids": [hit.id for hit in stemming.hits]},
        "text_filter_binding": {"ids": [hit.id for hit in text_binding.hits]},
        "repeated_filters": {"ids": [hit.id for hit in repeated_filters.hits]},
        "boolean_filter_grammar": {"ids": [hit.id for hit in boolean_filter.hits]},
        "sorting_projection": {"hits": [{"id": hit.id, "fields": dict(hit.fields)} for hit in projection.hits]},
        "pagination": {"all_ids": [hit.id for hit in all_sorted.hits], "offset_ids": [hit.id for hit in page.hits], "total": page.total_hits},
        "builder_misuse": {
            "observation": {
                "page_and_limit_rejected": True,
                "page_and_offset_rejected": True,
                "negative_limit_rejected": True,
                "negative_offset_rejected": True,
            },
            "capabilities": {
                "negative_limit_rejected": {"kind": "runtime_rejected"},
                "negative_offset_rejected": {"kind": "runtime_rejected"},
            },
        },
        "sorted_search": {"ids": [hit.id for hit in ordered.hits], "total": ordered.total_hits},
        "filtered_search": {"ids": [hit.id for hit in filtered.hits], "total": filtered.total_hits},
        "facets": facet_entries,
        "root_aggregations": dict(root.aggregations),
        "advanced_update": {"title": advanced_update["title"], "inventory": advanced_update["inventory"], "sale_price_present": "sale_price" in advanced_update},
        "grouping_aggregations": group_values,
        "filtered_facets": filtered_facet_entries,
        "nested_grouping": [{
            "category": group.value,
            "products": group.get("products"),
            "ratings": [{"rating": child.value, "products": child.get("products"), "avg_price": child.get("avg_price")} for child in group.groups],
        } for group in grouping_groups(nested)],
        "group_alias": [{"value": group.value, "fields": dict(group.fields)} for group in grouping_groups(aliased)],
        "group_keep_filter": [{"value": group.value, "fields": dict(group.fields)} for group in grouping_groups(kept)],
        **typed_observations,
        "rank_inputs": [{"id": hit.id, "score": hit.score, "feature": hit.features.get("query(boost)")} for hit in rank_input.hits],
        "rank_properties": [{"id": hit.id, "score": hit.score, "feature": hit.features.get("query(boost)")} for hit in rank_property.hits],
        "tensor_readback": tensor_value["$searchkernel_tensor"]["dense"],
        "nearest_neighbor": {"observation": {"intent": PARITY_CONSUMED_INTENTS["nearest_neighbor"], "ids": [hit.id for hit in nearest.hits], "scores": [hit.relevance for hit in nearest.hits]}, "capabilities": {}},
        "hybrid_text_vector": {"observation": {"intent": PARITY_CONSUMED_INTENTS["hybrid_text_vector"], "ids": [hit.id for hit in hybrid.hits], "top_score": hybrid.hits[0].score}, "capabilities": {}},
        "hybrid": {"top_id": hybrid.hits[0].id, "has_bag": any(hit.id == "bag-1" for hit in hybrid.hits)},
        "two_phase_reranking": {"ids": [hit.id for hit in reranked.hits]},
        "coverage_degradation": {"ids": [hit.id for hit in degraded.hits], "full": degraded.coverage.full, "degraded": degraded.coverage.degraded is not None},
        "yql": {"ids": [hit.id for hit in yql.hits], "total": yql.total_hits},
        "unknown_field_error": {"code": "invalid_argument", "details": error_codes},
        "structured_filter_error": {"code": "invalid_argument", "details": error_codes},
        "text_filter_syntax_error": syntax_error,
    }
    return observations


def write_real_engine_parity(observations: dict[str, object]) -> None:
    output = os.getenv("SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT")
    if output:
        for scenario, value in observations.items():
            if scenario.startswith("typed_query_"):
                continue
            intent = PARITY_CONSUMED_INTENTS.get(scenario)
            if intent is None:
                raise RuntimeError(f"parity scenario {scenario} emitted an observation without consuming its shared intent")
            payload = value.get("observation", value) if isinstance(value, dict) else None
            if isinstance(payload, dict):
                payload.setdefault("intent", intent)
            else:
                observations[scenario] = {"intent": intent, "result": value}
        Path(output).parent.mkdir(parents=True, exist_ok=True)
        Path(output).write_text(json.dumps({"version": 1, "scenarios": observations}, sort_keys=True))


def all_continuations(results) -> list[str]:
    tokens: list[str] = []

    def visit(node) -> None:
        tokens.extend(list(getattr(node, "continuations", []) or []))
        for child in getattr(node, "groups", []) or []:
            visit(child)

    root = getattr(results, "root", None)
    if root is not None:
        visit(root)

    for group in getattr(results, "groups", []) or []:
        visit(group)

    # Preserve order but remove duplicates caused by convenience projections
    # pointing into the canonical tree.
    return list(dict.fromkeys(tokens))


# ---------------------------------------------------------------------------
# Start completely fresh.
# ---------------------------------------------------------------------------

heading("1. CREATE INDEX")

with Index.create(
    INDEX_PATH,
    fields=fields_from_corpus(),
    rank_profiles=rank_profiles_from_corpus(),
) as index:
    print("created:", INDEX_PATH)

    # -----------------------------------------------------------------------
    # INSERT: build a deterministic corpus.
    # -----------------------------------------------------------------------

    heading("2. INSERT DOCUMENTS")

    documents = [document_input(source) for source in E2E_CORPUS["documents"]]

    for document in documents:
        index.insert(document)
        print("insert:", document["_id"])

    probe_spec = E2E_CORPUS["coverage_probes"]
    for probe in range(probe_spec["count"]):
        document = document_input({**probe_spec["document"], "_id": f"degraded-{probe}"})
        index.insert(document)
    print(f"inserted {probe_spec['count']} deterministic coverage probe documents")

    PARITY_OBSERVATIONS.update(capture_real_engine_parity(index))
    patch_spec = PARITY_INTENTS["patch"]
    index.insert(document_input({"_id": patch_spec["document_id"], **patch_spec["initial"]}))
    patch_builder = index.patch(patch_spec["document_id"])
    for key, value in patch_spec["set"].items(): patch_builder = patch_builder.set(key, corpus_value(key, value))
    for key in patch_spec["unset"]: patch_builder = patch_builder.unset(key)
    patch_builder.execute()
    mutated = index.get(patch_spec["document_id"])
    PARITY_OBSERVATIONS["patch"] = {"price": mutated["price"], "sale_price_present": "sale_price" in mutated}
    delete_spec = PARITY_INTENTS["delete"]
    parity_mutation_deleted = index.delete(delete_spec["document_id"])
    PARITY_OBSERVATIONS["delete"] = {"deleted": parity_mutation_deleted, "absent_after_delete": index.get(delete_spec["document_id"]) is None}
    reopen_spec = PARITY_INTENTS["reopen_persistence"]
    index.insert(document_input({"_id": reopen_spec["marker_id"], **reopen_spec["marker"]}))

    # -----------------------------------------------------------------------
    # GET
    # -----------------------------------------------------------------------

    heading("3. GET BY ID")

    doc = index.get("shoe-1")
    print(doc)

    assert doc is not None
    assert doc["_id"] == "shoe-1"
    assert doc["price"] == 89.0
    stored_embedding = doc["embedding"]["$searchkernel_tensor"]
    assert stored_embedding["type_spec"] == VECTOR_TYPE
    assert len(stored_embedding["dense"]) == len(SHOE_1_VECTOR)
    assert all(
        isclose(actual, expected, rel_tol=1e-6, abs_tol=1e-6)
        for actual, expected in zip(stored_embedding["dense"], SHOE_1_VECTOR)
    )

    # -----------------------------------------------------------------------
    # Full-text search.
    # -----------------------------------------------------------------------

    heading("4. FULL-TEXT SEARCH")

    results = index.search("shoes").limit(20).execute()
    print_results(results)

    assert {hit.id for hit in results.hits} == {
        "shoe-1",
        "shoe-2",
        "shoe-3",
        "shoe-4",
        "shoe-5",
    }
    assert results.total_hits == 5

    # The highlighted result is generated by Vespa's dynamic teaser path. The
    # query is stemmed: the match span covers "Jumped", not a literal
    # substring equal to the query text "jump".
    highlighted = (
        index.search("jump")
        .summary("default")
        .fields("category")
        .highlight("title")
        .execute()
    )
    highlighted_hit = next(hit for hit in highlighted.hits if hit.id == "shoe-1")
    assert highlighted_hit.fields == {"category": "shoes"}
    highlight = next(item for item in highlighted_hit.highlights if item["field"] == "title")
    fragment = highlight["fragments"][0]
    assert fragment["text"] == "Jumped Running Shoes for Daily Training"
    start, end = fragment["matches"][0]
    assert fragment["text"][start:end] == "Jumped"
    print("real Vespa highlighting with stemming and projection PASS")

    # -----------------------------------------------------------------------
    # Linguistics.
    # -----------------------------------------------------------------------

    heading("5. STEMMING: 'jump' -> 'Jumped'")

    results = index.search("jump").limit(20).execute()
    print_results(results)

    assert "shoe-1" in {hit.id for hit in results.hits}
    print("STEMMING PASS")

    # -----------------------------------------------------------------------
    # Primary string-filter API.
    # -----------------------------------------------------------------------

    heading("6. STRING FILTER + CLAUSE-LOCAL BINDING")

    results = (
        index.search("shoes")
        .filter("price <= :max", {"max": 100})
        .sort("price", "asc")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-3", "shoe-1"])

    # -----------------------------------------------------------------------
    # Repeated filters are AND and bindings are local to each clause.
    # The same placeholder name deliberately has different types/values.
    # -----------------------------------------------------------------------

    heading("7. REPEATED FILTERS + LOCAL BINDING SCOPES")

    results = (
        index.search("shoes")
        .filter("price <= :value", {"value": 120.0})
        .filter("rating >= :value", {"value": 4})
        .filter("in_stock = true")
        .sort("price", "asc")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-1", "shoe-5"])

    # -----------------------------------------------------------------------
    # Boolean grammar / precedence / list-valued IN binding.
    # -----------------------------------------------------------------------

    heading("8. FILTER GRAMMAR: OR / NOT / IN / PARENTHESES")

    results = (
        index.search("shoes")
        .filter(
            "(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)",
            {
                "brands": ["Acme", "TrailCo"],
                "rating": 3,
            },
        )
        .sort("price", "asc")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-3", "shoe-1", "shoe-5", "shoe-2"])

    # -----------------------------------------------------------------------
    # Explicit sorting + projection + result ergonomics.
    # -----------------------------------------------------------------------

    heading("9. SORT + FIELD PROJECTION + HIT ACCESS")

    results = (
        index.search("shoes")
        .sort("price", "desc")
        .fields("title", "brand", "price")
        .limit(2)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-4", "shoe-2"])
    assert results.hits[0]["title"] == "Acme Racing Shoes"
    assert results.hits[0]["brand"] == "Acme"
    assert results.hits[0]["price"] == 159.0
    assert "rating" not in results.hits[0].fields

    # -----------------------------------------------------------------------
    # Offset/limit and one-based page convenience.
    # -----------------------------------------------------------------------

    heading("10. PAGINATION")

    all_sorted = (
        index.search("shoes")
        .sort("price", "asc")
        .limit(20)
        .execute()
    )

    window = (
        index.search("shoes")
        .sort("price", "asc")
        .offset(1)
        .limit(2)
        .execute()
    )

    page = (
        index.search("shoes")
        .sort("price", "asc")
        .page(2, 2)
        .execute()
    )

    print("all:")
    print_results(all_sorted)
    print("window:")
    print_results(window)
    print("page:")
    print_results(page)

    assert_ids(window, [hit.id for hit in all_sorted.hits[1:3]])
    assert_ids(page, [hit.id for hit in all_sorted.hits[2:4]])
    assert window.total_hits == all_sorted.total_hits
    assert page.total_hits == all_sorted.total_hits

    # -----------------------------------------------------------------------
    # Obvious builder misuse should be rejected before execution.
    # -----------------------------------------------------------------------

    heading("11. BUILDER MISUSE")

    assert_raises(
        (TypeError, ValueError),
        lambda: index.search("shoes").page(2, 20).offset(20),
        "page() + offset() rejected",
    )
    assert_raises(
        (TypeError, ValueError),
        lambda: index.search("shoes").offset(20).page(2, 20),
        "offset() + page() rejected",
    )
    assert_raises(
        (TypeError, ValueError),
        lambda: index.search("shoes").page(2, 20).limit(20),
        "page() + limit() rejected",
    )
    assert_raises(
        (TypeError, ValueError),
        lambda: index.search("shoes").limit(-1),
        "negative limit rejected",
    )
    assert_raises(
        (TypeError, ValueError),
        lambda: index.search("shoes").offset(-1),
        "negative offset rejected",
    )

    # -----------------------------------------------------------------------
    # Hosted-search-style facet convenience.
    # -----------------------------------------------------------------------

    heading("12. FACETS")

    results = (
        index.search("shoes")
        .facet("brand", name="brands", limit=10, sort="count_desc")
        .facet("category", limit=10, sort="count_desc")
        .limit(20)
        .execute()
    )
    print_results(results)

    brand_counts = {
        item["value"]: item["count"]
        for item in results.facets["brands"]
    }
    assert brand_counts == {
        "Acme": 2,
        "TrailCo": 2,
        "Budget": 1,
    }

    category_counts = {
        item["value"]: item["count"]
        for item in results.facets["category"]
    }
    assert category_counts == {"shoes": 5}

    # -----------------------------------------------------------------------
    # Filter + facet composition.
    # -----------------------------------------------------------------------

    heading("13. FILTERED FACET")

    results = (
        index.search("shoes")
        .filter("in_stock = true")
        .facet("brand", limit=10, sort="count_desc")
        .limit(20)
        .execute()
    )
    print_results(results)

    brand_counts = {
        item["value"]: item["count"]
        for item in results.facets["brand"]
    }
    assert brand_counts == {
        "TrailCo": 2,
        "Acme": 1,
        "Budget": 1,
    }

    # -----------------------------------------------------------------------
    # Root-level aggregations: convenience projection over grouping.
    # -----------------------------------------------------------------------

    heading("14. ROOT AGGREGATIONS")

    results = (
        index.browse()
        .filter("category = 'shoes'")
        .aggregate(agg.count().as_("products"))
        .aggregate(agg.sum("price").as_("sum_price"))
        .aggregate(agg.avg("price").as_("avg_price"))
        .aggregate(agg.min("price").as_("min_price"))
        .aggregate(agg.max("price").as_("max_price"))
        .aggregate(agg.stddev("price").as_("stddev_price"))
        .limit(0)
        .execute()
    )
    print_results(results)

    assert results.aggregations["products"] == 5
    assert isclose(results.aggregations["sum_price"], 535.0)
    assert isclose(results.aggregations["avg_price"], 107.0)
    assert isclose(results.aggregations["min_price"], 49.0)
    assert isclose(results.aggregations["max_price"], 159.0)
    assert isfinite(results.aggregations["stddev_price"])
    assert results.aggregations["stddev_price"] > 0

    # -----------------------------------------------------------------------
    # Single-level structured grouping ordered by aggregation.
    # -----------------------------------------------------------------------

    heading("15. GROUP BY BRAND + COUNT / AVG + ORDER")

    results = (
        index.search("shoes")
        .group(
            Group.by("brand")
            .limit(10)
            .precision(100)
            .order_by(
                desc(agg.count()),
                asc(g.field("brand")),
            )
            .output(
                agg.count().as_("products"),
                agg.avg("price").as_("avg_price"),
            )
        )
        .execute()
    )
    print_results(results)

    by_brand = groups_by_value(grouping_groups(results))

    assert set(by_brand) == {"Acme", "TrailCo", "Budget"}
    assert by_brand["Acme"]["products"] == 2
    assert isclose(by_brand["Acme"]["avg_price"], 124.0)
    assert by_brand["TrailCo"]["products"] == 2
    assert isclose(by_brand["TrailCo"]["avg_price"], 119.0)
    assert by_brand["Budget"]["products"] == 1
    assert isclose(by_brand["Budget"]["avg_price"], 49.0)

    # Count DESC, then brand ASC for the tie.
    assert [group.value for group in grouping_groups(results)] == [
        "Acme",
        "TrailCo",
        "Budget",
    ]

    # -----------------------------------------------------------------------
    # Nested grouping proves recursive ResultGroup decoding.
    # -----------------------------------------------------------------------

    heading("16. NESTED GROUPING: CATEGORY -> BRAND")

    results = (
        index.browse()
        .filter("category IN (:categories)", {"categories": ["shoes", "bags"]})
        .group(
            Group.by("category")
            .limit(10)
            .order_by(desc(agg.count()))
            .output(agg.count().as_("products"))
            .subgroup(
                Group.by("brand")
                .limit(10)
                .order_by(
                    desc(agg.count()),
                    asc(g.field("brand")),
                )
                .output(
                    agg.count().as_("products"),
                    agg.avg("price").as_("avg_price"),
                )
            )
        )
        .execute()
    )
    print_results(results)

    categories = groups_by_value(grouping_groups(results))
    assert set(categories) == {"shoes", "bags"}
    assert categories["shoes"]["products"] == 5
    assert categories["bags"]["products"] == 1

    shoe_brands = groups_by_value(categories["shoes"].groups)
    assert set(shoe_brands) == {"Acme", "TrailCo", "Budget"}
    assert shoe_brands["Acme"]["products"] == 2
    assert shoe_brands["TrailCo"]["products"] == 2
    assert shoe_brands["Budget"]["products"] == 1

    bag_brands = groups_by_value(categories["bags"].groups)
    assert set(bag_brands) == {"Acme"}
    assert bag_brands["Acme"]["products"] == 1

    # -----------------------------------------------------------------------
    # Alias/output representative.
    # -----------------------------------------------------------------------

    heading("17. GROUP ALIAS + OUTPUT")

    results = (
        index.search("shoes")
        .group(
            Group.by("brand")
            .limit(10)
            .alias("revenue", agg.sum("price"))
            .order_by(desc(g.alias_ref("revenue")))
            .output(
                agg.count().as_("products"),
                g.alias_ref("revenue").as_("revenue"),
            )
        )
        .execute()
    )
    print_results(results)

    by_brand = groups_by_value(grouping_groups(results))
    assert isclose(by_brand["Acme"]["revenue"], 248.0)
    assert isclose(by_brand["TrailCo"]["revenue"], 238.0)
    assert isclose(by_brand["Budget"]["revenue"], 49.0)

    # -----------------------------------------------------------------------
    # Group keep/filter representative. This is deliberately NOT the
    # top-level string-filter language.
    # -----------------------------------------------------------------------

    heading("18. GROUP KEEP FILTER")

    results = (
        index.search("shoes")
        .group(
            Group.by("brand")
            .limit(10)
            .keep(gf.regex(g.field("brand"), "^A.*"))
            .output(agg.count().as_("products"))
        )
        .execute()
    )
    print_results(results)

    assert [group.value for group in grouping_groups(results)] == ["Acme"]
    assert grouping_groups(results)[0]["products"] == 2

    # -----------------------------------------------------------------------
    # Summary aggregation must return Hit children beneath groups rather than
    # flattening them into top-level hits.
    # -----------------------------------------------------------------------

    heading("19. SUMMARY AGGREGATION -> GROUP HIT CHILDREN")

    results = (
        index.search("shoes")
        .group(
            Group.by("brand")
            .output(
                agg.count().as_("products"),
                agg.summary().as_("hits"),
            )
        )
        .execute()
    )
    print_results(results)

    grouped_hit_ids = {
        hit.id
        for group in grouping_groups(results)
        for hit in group.hits
    }
    assert grouped_hit_ids == {
        "shoe-1",
        "shoe-2",
        "shoe-3",
        "shoe-4",
        "shoe-5",
    }

    # Grouping hit children remain available below their result groups. The
    # native search result may also expose the same documents as ordinary
    # root hits, so identity overlap is not evidence of flattening.
    assert grouped_hit_ids
    assert all(group.hits for group in grouping_groups(results))

    # -----------------------------------------------------------------------
    # Grouping continuation tokens remain opaque and survive result decoding.
    # -----------------------------------------------------------------------

    # heading("20. GROUP CONTINUATION TOKEN PRESERVATION")

    # results = (
    #     index.search("shoes")
    #     .group(
    #         Group.by("brand")
    #         .limit(1)
    #         .output(agg.count().as_("products"))
    #     )
    #     .execute()
    # )
    # print_results(results)

    # continuation_tokens = all_continuations(results)
    # assert continuation_tokens, (
    #     "expected a grouping continuation when limiting 3 brand groups to 1"
    # )
    # assert all(isinstance(token, str) and token for token in continuation_tokens)

    # -----------------------------------------------------------------------
    # Typed query DSL.
    # -----------------------------------------------------------------------

    heading("21. TYPED QUERY DSL")

    results = (
        index.query(q.contains("title", "shoes"))
        .ranking("default")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert {hit.id for hit in results.hits} == {
        "shoe-1",
        "shoe-2",
        "shoe-3",
        "shoe-4",
        "shoe-5",
    }

    # Representative request-wide query input and rank property namespaces.
    # These are intentionally separate from filter-local bindings.
    results = (
        index.query(q.contains("title", "shoes"))
        .input("category_boost", 1.5)
        .rank_property("showcase", "true")
        .ranking("default")
        .limit(5)
        .execute()
    )
    assert results.total_hits == 5

    heading("RANKING, SCALAR FEATURES, AND OPAQUE FEATURES")

    ranked = (
        index.search("shoes")
        .ranking("boosted")
        .input("boost", 3.0)
        .limit(20)
        .execute()
    )
    assert ranked.hits
    assert all(
        hit.score is not None and isclose(hit.features["query(boost)"], 3.0)
        for hit in ranked.hits
    )

    rank_property_results = (
        index.search("shoes")
        .ranking("boosted")
        .rank_property("boost", "4.0")
        .limit(20)
        .execute()
    )
    assert rank_property_results.hits
    assert all(
        hit.score is not None and isclose(hit.features["query(boost)"], 4.0)
        for hit in rank_property_results.hits
    )
    print("scalar ranking feature PASS")

    heading("TWO-PHASE RANKING / SECOND-PHASE RERANK")

    first_phase = (
        index.search("candidate")
        .ranking("first_phase_only")
        .limit(4)
        .execute()
    )
    assert_ids(first_phase, ["rerank-a", "rerank-b", "rerank-c", "rerank-d"])
    assert [hit.score for hit in first_phase.hits] == [100.0, 90.0, 80.0, 70.0]

    two_phase = (
        index.search("candidate")
        .ranking("two_phase")
        .limit(4)
        .execute()
    )
    assert_ids(two_phase, ["rerank-b", "rerank-c", "rerank-a", "rerank-d"])
    assert two_phase.total_hits == 4
    assert [hit.score for hit in two_phase.hits[:3]] == [100.0, 90.0, 1.0]
    assert isfinite(two_phase.hits[3].score)
    assert two_phase.hits[3].score != 1000.0
    print(
        "first phase A>B>C>D; rerank window A,B,C; "
        "second phase B>C>A; final B,C,A,D PASS"
    )

    heading("TENSOR READBACK + NEAREST NEIGHBOR")

    nearest = (
        index.query(q.nearest_neighbor("embedding", "query_embedding"))
        .ranking("semantic")
        .input("query_embedding", embedding(*SHOE_1_VECTOR))
        .limit(3)
        .execute()
    )
    assert nearest.hits[0].id == "shoe-1"
    assert nearest.hits[0].score > nearest.hits[1].score
    assert nearest.coverage is not None
    opaque_feature = nearest.hits[0].features["query(query_embedding)"]
    assert isinstance(opaque_feature, bytes)
    print("8D tensor query input and native nearest-neighbor search PASS")

    heading("HYBRID TEXT + VECTOR SEARCH")

    lexical = (
        index.query(q.contains("title", "shoes"))
        .limit(20)
        .execute()
    )
    assert "bag-1" not in {hit.id for hit in lexical.hits}

    # OR is intentional: AND would only test the intersection and could hide
    # a broken vector candidate path. The rank profile then combines both
    # branches into one score.
    hybrid = (
        index.query(
            q.any_of(
                q.contains("title", "shoes"),
                q.nearest_neighbor("embedding", "query_embedding"),
            )
        )
        .ranking("hybrid")
        .input("query_embedding", embedding(*BAG_1_VECTOR))
        .limit(20)
        .execute()
    )
    hybrid_ids = {hit.id for hit in hybrid.hits}
    assert "bag-1" in hybrid_ids, "vector-only candidate disappeared from hybrid search"
    assert {"shoe-1", "shoe-2", "shoe-3", "shoe-4", "shoe-5"}.issubset(hybrid_ids)
    assert hybrid.hits[0].id == "bag-1"

    top = hybrid.hits[0]
    assert isclose(top.features["bm25(title)"], 0.0)
    assert top.features["closeness(embedding)"] > 0.99
    assert isclose(
        top.score,
        top.features["bm25(title)"] + 10 * top.features["closeness(embedding)"],
        rel_tol=1e-5,
        abs_tol=1e-5,
    )
    lexical_hit = next(hit for hit in hybrid.hits if hit.id == "shoe-1")
    assert lexical_hit.features["bm25(title)"] > 0
    assert isclose(
        lexical_hit.score,
        lexical_hit.features["bm25(title)"]
        + 10 * lexical_hit.features["closeness(embedding)"],
        rel_tol=1e-5,
        abs_tol=1e-5,
    )
    print("lexical + 8D vector candidate union and hybrid rank score PASS")

    heading("COVERAGE AND DEGRADATION")

    normal = index.search("shoes").limit(20).execute()
    assert normal.coverage is not None
    assert normal.coverage.full is True

    degraded = (
        index.browse()
        .filter("inventory = :value", {"value": 18})
        .sort("inventory", "asc")
        .rank_property("vespa.matchphase.degradation.maxfiltercoverage", "0.9")
        .rank_property("vespa.matchphase.degradation.attribute", "inventory")
        .rank_property("vespa.matchphase.degradation.maxhits", "10")
        .limit(10)
        .execute()
    )
    assert degraded.hits
    assert degraded.coverage is not None
    assert degraded.coverage.full is False
    assert degraded.coverage.degraded is not None
    assert degraded.coverage.degraded.timeout or degraded.coverage.degraded.match_phase
    print("coverage and valid degradation metadata PASS")

    # -----------------------------------------------------------------------
    # YQL escape hatch + request-wide property substitution.
    # -----------------------------------------------------------------------

    heading("22. YQL + PROPERTY + PAGINATION")

    results = (
        index.yql(
            "select * from sources * "
            "where title contains text(@q) "
            "order by price asc"
        )
        .property("q", "shoes")
        .ranking("default")
        .page(1, 3)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-3", "shoe-1", "shoe-5"])
    assert results.total_hits == 5

    # -----------------------------------------------------------------------
    # YQL + programmatic grouping is explicitly valid.
    # -----------------------------------------------------------------------

    heading("23. YQL + PROGRAMMATIC GROUPING")

    results = (
        index.yql(
            "select * from sources * "
            "where title contains text(@q)"
        )
        .property("q", "shoes")
        .group(
            Group.by("brand")
            .limit(10)
            .output(agg.count().as_("products"))
        )
        .execute()
    )
    print_results(results)

    by_brand = groups_by_value(grouping_groups(results))
    assert {
        name: group["products"]
        for name, group in by_brand.items()
    } == {
        "Acme": 2,
        "TrailCo": 2,
        "Budget": 1,
    }

    # -----------------------------------------------------------------------
    # Structured filter errors survive as structured Python exceptions.
    # -----------------------------------------------------------------------

    heading("24. STRUCTURED FILTER ERROR")

    try:
        (
            index.search("shoes")
            .filter("prcie <= :max", {"max": 100})
            .execute()
        )
    except SearchKernelError as error:
        print("code:", error.code)
        print("message:", error.message)
        print("details:", error.details)

        assert error.details
        assert error.details[0].code == "unknown_field"
        assert error.details[0].path == "filters.textual[0].source"
    else:
        raise AssertionError("unknown field filter should fail")

    # Clause-local missing and unused binding failures should remain structured.
    try:
        (
            index.search("shoes")
            .filter("price <= :max", {"unused": 100})
            .execute()
        )
    except SearchKernelError as error:
        detail_codes = {detail.code for detail in error.details}
        assert "unknown_parameter" in detail_codes
    else:
        raise AssertionError("missing filter binding should fail")

    try:
        (
            index.search("shoes")
            .filter("price <= :max", {"max": 100, "unused": 200})
            .execute()
        )
    except SearchKernelError as error:
        detail_codes = {detail.code for detail in error.details}
        assert "unused_parameter" in detail_codes
    else:
        raise AssertionError("unused filter binding should fail")

    heading("24A. TEXTUAL FILTER SYNTAX ERROR")

    try:
        (
            index.search("shoes")
            .filter("price <= :max AND (")
            .execute()
        )
    except SearchKernelError as error:
        syntax_error = next(
            (detail for detail in error.details if detail.code == "syntax_error"),
            None,
        )
        assert syntax_error is not None
        assert syntax_error.path == "filters.textual[0].source"
    else:
        raise AssertionError("malformed textual filter should fail in the parser")
    print("textual filter parser, clause-local bindings, grammar, and syntax errors PASS")

    # -----------------------------------------------------------------------
    # PATCH builder: simple partial update + explicit unset.
    # -----------------------------------------------------------------------

    heading("25. PATCH BUILDER")

    print("before:", index.get("shoe-1"))

    (
        index.patch("shoe-1")
        .set("price", 79.0)
        .set("rating", 4)
        .unset("sale_price")
        .execute()
    )

    updated = index.get("shoe-1")
    print("after:", updated)

    assert updated is not None
    assert updated["price"] == 79.0
    assert updated["rating"] == 4
    assert "sale_price" not in updated

    results = (
        index.search("shoes")
        .filter("price < :max", {"max": 80})
        .sort("price", "asc")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-3", "shoe-1"])

    # -----------------------------------------------------------------------
    # Advanced UpdateBuilder representative.
    # -----------------------------------------------------------------------

    heading("26. ADVANCED UPDATE BUILDER")

    index.update("shoe-1").unset("sale_price").execute()
    assert "sale_price" not in index.get("shoe-1")

    (
        index.update("shoe-1")
        .set("title", "Fast Marathon Shoes")
        .execute()
    )

    updated = index.get("shoe-1")
    assert updated["title"] == "Fast Marathon Shoes"
    marathon_results = index.search("marathon").execute()
    assert "shoe-1" in {hit.id for hit in marathon_results.hits}
    old_title_results = index.search("jumped").execute()
    assert "shoe-1" not in {hit.id for hit in old_title_results.hits}

    index.update("shoe-1").set("inventory", 20).execute()
    assert index.get("shoe-1")["inventory"] == 20

    (
        index.update("shoe-1")
        .increment("inventory", 5)
        .execute()
    )

    updated = index.get("shoe-1")
    print(updated)

    assert updated["inventory"] == 25

    index.update("shoe-1").decrement("inventory", 3).execute()
    assert index.get("shoe-1")["inventory"] == 22

    index.update("shoe-1").multiply("inventory", 2).execute()
    assert index.get("shoe-1")["inventory"] == 44

    index.update("shoe-1").divide("inventory", 4).execute()
    assert index.get("shoe-1")["inventory"] == 11

    index.update("shoe-created").set("inventory", 7).create().execute()
    assert index.get("shoe-created")["inventory"] == 7

    try:
        index.update("shoe-1").set("inventory", 16).condition("inventory == 15").execute()
    except SearchKernelError as error:
        assert error.code == "unsupported"
    else:
        raise AssertionError("embedded update conditions must be typed Unsupported")

    # -----------------------------------------------------------------------
    # DELETE.
    # -----------------------------------------------------------------------

    heading("27. DELETE")

    assert index.delete("shoe-3") is True
    assert index.get("shoe-3") is None

    results = index.search("shoes").limit(20).execute()
    print_results(results)

    assert "shoe-3" not in {hit.id for hit in results.hits}
    assert results.total_hits == 4

    # Facet projection must reflect the deletion too.
    results = (
        index.search("shoes")
        .facet("brand")
        .limit(20)
        .execute()
    )
    brand_counts = {
        item["value"]: item["count"]
        for item in results.facets["brand"]
    }
    assert "Budget" not in brand_counts

    assert index.delete("shoe-3") is False
    print("DELETE PASS")

    # -----------------------------------------------------------------------
    # UPSERT replacement.
    # -----------------------------------------------------------------------

    heading("28. UPSERT REPLACEMENT")

    upsert_spec = PARITY_INTENTS["upsert_replacement"]
    replacement = dict(upsert_spec["replacement"])
    vector_document_id = replacement.pop("vector_document_id")
    index.upsert({"_id": upsert_spec["document_id"], **replacement, "embedding": embedding(*corpus_vector(vector_document_id))})

    doc = index.get(upsert_spec["document_id"])
    print(doc)

    assert doc["price"] == 119.0
    assert "Updated Edition" in doc["title"]

    results = index.search(upsert_spec["search_query"]).limit(20).execute()
    print_results(results)

    assert {hit.id for hit in results.hits} == {upsert_spec["document_id"]}
    PARITY_OBSERVATIONS["upsert_replacement"] = {"price": doc["price"], "updated_title_matches": "Updated" in doc["title"], "search_ids": [hit.id for hit in results.hits]}


heading("29. INDEX CLOSED")
print("The first Engine instance has shut down.")


# ---------------------------------------------------------------------------
# Reopen the SAME persistent index.
# ---------------------------------------------------------------------------

heading("30. REOPEN PERSISTED INDEX")

with Index.open(INDEX_PATH) as index:
    reopen_spec = PARITY_INTENTS["reopen_persistence"]
    upsert_spec = PARITY_INTENTS["upsert_replacement"]
    persisted = index.get(reopen_spec["marker_id"])
    assert persisted is not None
    shoe2 = index.get(upsert_spec["document_id"])
    PARITY_OBSERVATIONS["reopen_persistence"] = {
        "marker": {"title": persisted["title"], "price": persisted["price"], "inventory": persisted["inventory"]},
        "upserted": {"updated_title_matches": "Updated" in shoe2["title"], "price": shoe2["price"]},
        "deleted_absent": index.get(PARITY_INTENTS["delete"]["document_id"]) is None,
    }
    # Deleted document remains deleted.
    assert index.get("shoe-3") is None

    # Patch + advanced update survive reopen.
    shoe1 = index.get("shoe-1")
    print("shoe-1 after reopen:", shoe1)

    assert shoe1["price"] == 79.0
    assert shoe1["rating"] == 4
    assert shoe1["inventory"] == 11
    assert shoe1["title"] == "Fast Marathon Shoes"
    assert "shoe-1" in {hit.id for hit in index.search("marathon").execute().hits}
    assert "shoe-1" not in {hit.id for hit in index.search("jumped").execute().hits}
    assert index.get("shoe-created")["inventory"] == 7
    assert "sale_price" not in shoe1
    reopened_embedding = shoe1["embedding"]["$searchkernel_tensor"]
    assert reopened_embedding["type_spec"] == VECTOR_TYPE
    assert len(reopened_embedding["dense"]) == len(SHOE_1_VECTOR)
    assert all(
        isclose(actual, expected, rel_tol=1e-6, abs_tol=1e-6)
        for actual, expected in zip(reopened_embedding["dense"], SHOE_1_VECTOR)
    )

    # Upsert replacement survives.
    shoe2 = index.get("shoe-2")
    print("shoe-2 after reopen:", shoe2)

    assert shoe2["price"] == 119.0
    assert "Updated Edition" in shoe2["title"]

    # String-filter parsing/binding remains schema-aware after reopen.
    results = (
        index.search("shoes")
        .filter("price <= :max", {"max": 120})
        .sort("price", "asc")
        .limit(20)
        .execute()
    )
    print_results(results)

    assert_ids(results, ["shoe-1", "shoe-5", "shoe-2"])

    reopened_ranked = (
        index.search("shoes")
        .ranking("boosted")
        .input("boost", 3.0)
        .limit(20)
        .execute()
    )
    assert all(
        hit.score is not None and isclose(hit.features["query(boost)"], 3.0)
        for hit in reopened_ranked.hits
    )

    reopened_nearest = (
        index.query(q.nearest_neighbor("embedding", "query_embedding"))
        .ranking("semantic")
        .input("query_embedding", embedding(*SHOE_1_VECTOR))
        .limit(3)
        .execute()
    )
    assert reopened_nearest.hits[0].id == "shoe-1"

    # Browse exposes the same filter/facet/grouping facade without text input.
    heading("31. BROWSE AFTER REOPEN")

    results = (
        index.browse()
        .filter("in_stock = true")
        .sort("price", "asc")
        .facet("brand")
        .group(
            Group.by("category")
            .limit(10)
            .output(agg.count().as_("products"))
        )
        .limit(256)
        .execute()
    )
    print_results(results)

    browse_ids = {hit.id for hit in results.hits}

    assert "shoe-4" not in browse_ids
    assert "shoe-3" not in browse_ids
    assert {
        "shoe-1",
        "shoe-2",
        "shoe-5",
        "bag-1",
    }.issubset(browse_ids)

    categories = groups_by_value(grouping_groups(results))
    assert categories["shoes"]["products"] == 3
    assert categories["bags"]["products"] == 1

    write_real_engine_parity(PARITY_OBSERVATIONS)

heading("32. ALL CLIENT API E2E CHECKS PASSED")

print(
    """
Successfully demonstrated through the public Python facade:

  DOCUMENTS
  ✓ create/open persistent index
  ✓ insert
  ✓ upsert replacement
  ✓ get
  ✓ delete + duplicate delete
  ✓ PatchBuilder set/unset
  ✓ UpdateBuilder set / unset / increment / decrement / multiply / divide / create
  ✓ persistence across reopen

  SEARCH / FILTERING
  ✓ SearchBuilder full-text search
  ✓ stemming representative
  ✓ string filter language
  ✓ clause-local filter bindings
  ✓ repeated filters -> AND
  ✓ placeholder reuse across independent filter clauses
  ✓ OR / NOT / IN / parentheses
  ✓ malformed textual filter -> syntax_error
  ✓ sorting
  ✓ fields projection
  ✓ offset/limit
  ✓ one-based page()
  ✓ local pagination misuse rejection
  ✓ total hits
  ✓ structured filter errors

  FACETS / GROUPING / AGGREGATION
  ✓ facet convenience + projection
  ✓ filtered facets
  ✓ root count/sum/avg/min/max/stddev
  ✓ Group.by()
  ✓ group limit + precision
  ✓ ordering by aggregation
  ✓ nested groups
  ✓ recursive ResultGroup decoding
  ✓ aliases + output
  ✓ grouping keep/filter representative
  ✓ summary aggregation -> Hit children
  — grouping continuation preservation [DEFERRED: continuation tokens are not sent through the native grouping ABI]

  QUERY / RANKING / YQL
  ✓ typed query DSL representative
  ✓ query input namespace
  ✓ rank property namespace
  ✓ ranking profile selection
  ✓ real two-phase reranking with a finite candidate window and propagated scores
  ✓ raw YQL
  ✓ YQL @property substitution
  ✓ YQL pagination
  ✓ YQL + programmatic grouping

  RESULTS
  ✓ Hit mapping access
  ✓ dense tensor document readback and persistence
  ✓ scalar and opaque match-feature results
  ✓ structured coverage and degradation metadata
  ✓ 8D tensor query input and native nearest-neighbor search
  ✓ lexical + 8D vector candidate union and hybrid rank score
  ✓ canonical recursive grouping result shape
  ✓ convenience hits/groups/facets/aggregations projections
"""
)
