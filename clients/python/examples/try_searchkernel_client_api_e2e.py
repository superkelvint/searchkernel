from __future__ import annotations

from math import isclose, isfinite
from pathlib import Path
import atexit
import shutil
import tempfile

from searchkernel import (
    Index,
    Text,
    Keyword,
    Integer,
    Float,
    Boolean,
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
    fields={
        "title": Text(
            stored=True,
            searchable=True,
        ),
        "brand": Keyword(
            stored=True,
            filterable=True,
            sortable=True,
            facetable=True,
        ),
        "category": Keyword(
            stored=True,
            filterable=True,
            facetable=True,
        ),
        "price": Float(
            stored=True,
            filterable=True,
            sortable=True,
        ),
        "sale_price": Float(
            stored=True,
            filterable=True,
            sortable=True,
        ),
        "rating": Integer(
            stored=True,
            filterable=True,
            sortable=True,
        ),
        "inventory": Integer(
            stored=True,
            filterable=True,
            sortable=True,
        ),
        "in_stock": Boolean(
            stored=True,
            filterable=True,
        ),
        "embedding": TensorField("tensor<float>(x[2])", stored=True),
    },
    rank_profiles=[
        RankProfile("boosted")
        .first_phase("query(boost)")
        .match_feature("query(boost)"),
        RankProfile("semantic")
        .first_phase("closeness(embedding)")
        .query_input("query_embedding", "tensor<float>(x[2])")
        .match_feature("query(query_embedding)"),
    ],
) as index:
    print("created:", INDEX_PATH)

    # -----------------------------------------------------------------------
    # INSERT: build a deterministic corpus.
    # -----------------------------------------------------------------------

    heading("2. INSERT DOCUMENTS")

    documents = [
        {
            "_id": "shoe-1",
            "title": "Jumped Running Shoes for Daily Training",
            "brand": "Acme",
            "category": "shoes",
            "price": 89.0,
            "sale_price": 79.0,
            "rating": 5,
            "inventory": 10,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [1.0, 0.0]),
        },
        {
            "_id": "shoe-2",
            "title": "Trail Running Shoes",
            "brand": "TrailCo",
            "category": "shoes",
            "price": 129.0,
            "rating": 5,
            "inventory": 4,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.8, 0.2]),
        },
        {
            "_id": "shoe-3",
            "title": "Budget Walking Shoes",
            "brand": "Budget",
            "category": "shoes",
            "price": 49.0,
            "rating": 3,
            "inventory": 25,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.0, 1.0]),
        },
        {
            "_id": "shoe-4",
            "title": "Acme Racing Shoes",
            "brand": "Acme",
            "category": "shoes",
            "price": 159.0,
            "rating": 4,
            "inventory": 0,
            "in_stock": False,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.7, 0.3]),
        },
        {
            "_id": "shoe-5",
            "title": "TrailCo Hiking Shoes",
            "brand": "TrailCo",
            "category": "shoes",
            "price": 109.0,
            "rating": 4,
            "inventory": 8,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.9, 0.1]),
        },
        {
            "_id": "bag-1",
            "title": "Lightweight Running Backpack",
            "brand": "Acme",
            "category": "bags",
            "price": 69.0,
            "rating": 4,
            "inventory": 12,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.6, 0.4]),
        },
    ]

    for document in documents:
        index.insert(document)
        print("insert:", document["_id"])

    for probe in range(200):
        index.insert(
            {
                "_id": f"degraded-{probe}",
                "title": "Coverage Degradation Probe",
                "category": "degradation",
                "price": 18.0,
                "rating": 1,
                "inventory": 18,
                "in_stock": True,
                "embedding": Tensor.dense("tensor<float>(x[2])", [0.0, 1.0]),
            }
        )
    print("inserted 200 deterministic coverage probe documents")

    # -----------------------------------------------------------------------
    # GET
    # -----------------------------------------------------------------------

    heading("3. GET BY ID")

    doc = index.get("shoe-1")
    print(doc)

    assert doc is not None
    assert doc["_id"] == "shoe-1"
    assert doc["price"] == 89.0
    assert doc["embedding"] == {
        "$searchkernel_tensor": {
            "type_spec": "tensor<float>(x[2])",
            "dense": [1.0, 0.0],
        }
    }

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

    by_brand = groups_by_value(results.groups)

    assert set(by_brand) == {"Acme", "TrailCo", "Budget"}
    assert by_brand["Acme"]["products"] == 2
    assert isclose(by_brand["Acme"]["avg_price"], 124.0)
    assert by_brand["TrailCo"]["products"] == 2
    assert isclose(by_brand["TrailCo"]["avg_price"], 119.0)
    assert by_brand["Budget"]["products"] == 1
    assert isclose(by_brand["Budget"]["avg_price"], 49.0)

    # Count DESC, then brand ASC for the tie.
    assert [group.value for group in results.groups] == [
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

    categories = groups_by_value(results.groups)
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

    by_brand = groups_by_value(results.groups)
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

    assert [group.value for group in results.groups] == ["Acme"]
    assert results.groups[0]["products"] == 2

    # -----------------------------------------------------------------------
    # Summary aggregation must return Hit children beneath groups rather than
    # flattening them into top-level hits.
    # -----------------------------------------------------------------------

    # heading("19. SUMMARY AGGREGATION -> GROUP HIT CHILDREN")

    # results = (
    #     index.search("shoes")
    #     .group(
    #         Group.by("brand")
    #         .limit(10)
    #         .output(
    #             agg.count().as_("products"),
    #             agg.summary().as_("hits"),
    #         )
    #     )
    #     .execute()
    # )
    # print_results(results)

    # grouped_hit_ids = {
    #     hit.id
    #     for group in results.groups
    #     for hit in group.hits
    # }
    # assert grouped_hit_ids == {
    #     "shoe-1",
    #     "shoe-2",
    #     "shoe-3",
    #     "shoe-4",
    #     "shoe-5",
    # }

    # # Grouping hit children are not silently flattened into top-level hits.
    # assert {hit.id for hit in results.hits}.isdisjoint(grouped_hit_ids)

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

    heading("TENSOR READBACK + NEAREST NEIGHBOR")

    nearest = (
        index.query(q.nearest_neighbor("embedding", "query_embedding"))
        .ranking("semantic")
        .input("query_embedding", Tensor.dense("tensor<float>(x[2])", [1.0, 0.0]))
        .limit(3)
        .execute()
    )
    assert nearest.hits[0].id == "shoe-1"
    assert nearest.coverage is not None
    opaque_feature = nearest.hits[0].features["query(query_embedding)"]
    assert isinstance(opaque_feature, bytes)
    print("tensor query input, native nearest-neighbor search, and opaque feature PASS")

    heading("HYBRID TEXT + VECTOR SEARCH")

    hybrid = (
        index.query(
            q.and_(
                q.contains("title", "shoes"),
                q.nearest_neighbor("embedding", "query_embedding"),
            )
        )
        .ranking("semantic")
        .input("query_embedding", Tensor.dense("tensor<float>(x[2])", [1.0, 0.0]))
        .limit(3)
        .execute()
    )
    assert hybrid.hits[0].id == "shoe-1"
    assert "shoe-1" in {hit.id for hit in hybrid.hits}
    assert "bag-1" not in {hit.id for hit in hybrid.hits}
    print("text-and-vector hybrid search PASS")

    heading("COVERAGE AND DEGRADATION")

    normal = index.search("shoes").limit(20).execute()
    assert normal.coverage is not None
    assert normal.coverage.full is True

    degraded = (
        index.search("")
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

    by_brand = groups_by_value(results.groups)
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

    index.upsert(
        {
            "_id": "shoe-2",
            "title": "Trail Running Shoes - Updated Edition",
            "brand": "TrailCo",
            "category": "shoes",
            "price": 119.0,
            "rating": 5,
            "inventory": 4,
            "in_stock": True,
            "embedding": Tensor.dense("tensor<float>(x[2])", [0.8, 0.2]),
        }
    )

    doc = index.get("shoe-2")
    print(doc)

    assert doc["price"] == 119.0
    assert "Updated Edition" in doc["title"]

    results = index.search("updated").limit(20).execute()
    print_results(results)

    assert {hit.id for hit in results.hits} == {"shoe-2"}


heading("29. INDEX CLOSED")
print("The first Engine instance has shut down.")


# ---------------------------------------------------------------------------
# Reopen the SAME persistent index.
# ---------------------------------------------------------------------------

heading("30. REOPEN PERSISTED INDEX")

with Index.open(INDEX_PATH) as index:
    # Deleted document remains deleted.
    assert index.get("shoe-3") is None

    # Patch + advanced update survive reopen.
    shoe1 = index.get("shoe-1")
    print("shoe-1 after reopen:", shoe1)

    assert shoe1["price"] == 79.0
    assert shoe1["rating"] == 4
    assert shoe1["inventory"] == 25
    assert shoe1["title"] == "Fast Marathon Shoes"
    assert "shoe-1" in {hit.id for hit in index.search("marathon").execute().hits}
    assert "shoe-1" not in {hit.id for hit in index.search("jumped").execute().hits}
    assert index.get("shoe-created")["inventory"] == 7
    assert "sale_price" not in shoe1
    assert shoe1["embedding"] == {
        "$searchkernel_tensor": {
            "type_spec": "tensor<float>(x[2])",
            "dense": [1.0, 0.0],
        }
    }

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
        .input("query_embedding", Tensor.dense("tensor<float>(x[2])", [1.0, 0.0]))
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

    categories = groups_by_value(results.groups)
    assert categories["shoes"]["products"] == 3
    assert categories["bags"]["products"] == 1


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
  ✓ UpdateBuilder set / unset / increment / create
  ✓ persistence across reopen

  SEARCH / FILTERING
  ✓ SearchBuilder full-text search
  ✓ stemming representative
  ✓ string filter language
  ✓ clause-local filter bindings
  ✓ repeated filters -> AND
  ✓ placeholder reuse across independent filter clauses
  ✓ OR / NOT / IN / parentheses
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
  — summary aggregation -> Hit children [BLOCKED: native docsum payloads are not decoded]
  — grouping continuation preservation [DEFERRED: continuation tokens are not sent through the native grouping ABI]

  QUERY / RANKING / YQL
  ✓ typed query DSL representative
  ✓ query input namespace
  ✓ rank property namespace
  ✓ ranking profile selection
  ✓ raw YQL
  ✓ YQL @property substitution
  ✓ YQL pagination
  ✓ YQL + programmatic grouping

  RESULTS
  ✓ Hit mapping access
  ✓ dense tensor document readback and persistence
  ✓ scalar and opaque match-feature results
  ✓ structured coverage and degradation metadata
  ✓ tensor query input and native nearest-neighbor search
  ✓ text-and-vector hybrid search
  ✓ canonical recursive grouping result shape
  ✓ convenience hits/groups/facets/aggregations projections
"""
)
