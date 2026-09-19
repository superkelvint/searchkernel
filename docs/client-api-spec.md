# Search Kernel Client API Specification

**Status:** Proposed specification  
**Audience:** Search Kernel SDK, protocol, IR, transport, backend, and documentation implementers  
**Scope:** Cross-language public client facade and result model  
**Out of scope:** Filter-expression grammar internals, transport framing, generated protocol APIs, server/index lifecycle administration

---

## 1. Purpose

Search Kernel needs a public client API that is exceptionally easy to learn for developers coming from Algolia, Typesense, Elasticsearch, database clients, and ordinary application SDKs, while retaining access to the capabilities of Vespa's search engine.

The public API should optimize for this progression:

```text
simple things are obvious
        ↓
common advanced things stay fluent
        ↓
expert capabilities remain typed and composable
        ↓
raw Vespa escape hatches exist when needed
```

A developer should be productive after seeing this once:

```python
results = (
    index.search("running shoes")
    .filter("price <= :max AND in_stock = true", {"max": 120})
    .sort("price", "asc")
    .facet("brand")
    .page(1, 20)
    .execute()
)
```

The same client must also be capable of expressing substantially richer searches without forcing the user to construct FlatBuffers objects or write raw YQL:

```python
results = (
    index.search("running shoes")
    .filter("in_stock = true")
    .ranking("product-search")
    .input("user_embedding", embedding)
    .group(
        Group.by("category")
        .limit(20)
        .order_by(desc(agg.count()))
        .output(
            agg.count().as_("products"),
            agg.avg("price").as_("avg_price"),
        )
        .subgroup(
            Group.by("brand")
            .limit(10)
            .output(agg.count().as_("products"))
        )
    )
    .fields("title", "brand", "price")
    .limit(20)
    .execute()
)
```

The public facade is ergonomic application API. The canonical FlatBuffers contract remains the normative portable representation underneath it.

---

# Part I — Design Principles

## 2. Fluent builders are the standard

The fluent builder style is normative for configurable operations.

Primary entry points:

```text
index.search(text)       -> SearchBuilder
index.browse()           -> SearchBuilder
index.query(expr)        -> SearchBuilder
index.yql(source)        -> YqlBuilder
index.patch(id)          -> PatchBuilder
index.update(id)         -> UpdateBuilder
index.analyze(text)      -> AnalyzeBuilder
```

Operations that are already complete commands may execute directly:

```text
insert(document)
upsert(document)
get(id)
delete(id)
describe()
```

Do not add `.execute()` merely for stylistic uniformity where there is no request to configure.

---

## 3. Ergonomics first, protocol second

Normal users should work with:

```text
Index
SearchBuilder
YqlBuilder
PatchBuilder
UpdateBuilder
Query
Group
Aggregation
SearchResult
Hit
ResultGroup
```

They should not normally construct generated protocol types such as:

```text
SearchRequest
QueryExpr
GroupingOperation
GroupingExpression
ResultNodeKind
ValueEntry
```

Facade objects may closely correspond to protocol concepts, but they are deliberately smaller, idiomatic, discoverable, and designed around application usage.

The rule is:

> Generated protocol types are the wire contract. Facade types are the developer experience.

---

## 4. Progressive disclosure

Search Kernel should expose four levels of search power.

### Level 1 — Familiar search

```python
index.search("wireless headphones")
    .filter("price < 300")
    .sort("price", "asc")
    .facet("brand")
    .limit(20)
    .execute()
```

This should feel natural to users of hosted search products.

### Level 2 — Advanced request controls

```python
index.search("wireless headphones")
    .search_field("title", weight=3)
    .search_field("description", weight=1)
    .ranking("product-search")
    .input("user_embedding", embedding)
    .summary("search-result")
    .highlight("title")
    .execute()
```

### Level 3 — Typed query and structured grouping

```python
index.query(
    q.and_(
        q.contains("title", q.phrase("wireless", "headphones")),
        q.nearest_neighbor("embedding", "user_embedding"),
    )
)
.input("user_embedding", embedding)
.group(...)
.execute()
```

### Level 4 — Vespa escape hatches

```python
index.yql("select ...")
```

and, where needed:

```python
.raw_grouping("all(group(category) each(output(count())))")
.raw_sort("...")
.property("vespa.query.property", value)
```

The presence of advanced capabilities must not make basic search harder to understand.

---

## 5. One semantic implementation

SDKs normalize ergonomic calls into the canonical portable request.

They do not independently implement Search Kernel semantics.

```text
language facade
      ↓
portable FlatBuffers request
      ↓
canonical dispatcher
      ↓
API -> IR
      ↓
backend adapter
      ↓
Vespa
```

This applies especially to:

- filter parsing and binding;
- defaults;
- schema validation;
- query validation;
- grouping semantics;
- result semantics;
- ranking semantics;
- portable errors.

Client-side validation should catch trivial API misuse early, but must not become a competing semantic implementation.

---

## 6. Cross-language vocabulary

Use the same conceptual names wherever host-language syntax permits:

```text
search
browse
query
yql
filter
param
search_field
sort
facet
group
aggregate
ranking
input
fields
summary
highlight
offset
limit
page
timeout
execute
inspect
```

Host languages may adapt casing and reserved words.

The contract is semantic, not source-code character-for-character identity.

---

# Part II — Index and Document Operations

## 7. Index handle

This specification begins with an acquired `Index` handle.

How an SDK connects to a Search Kernel daemon, selects an index, or opens an embedded index is transport/deployment-specific and is intentionally outside this document.

Examples therefore assume:

```python
index = ...
```

Once obtained, `Index` exposes the same search/document semantics regardless of transport.

---

## 8. Insert, upsert, get, delete

Canonical document operations:

```python
index.insert(document)
index.upsert(document)
index.get("sku-42")
index.delete("sku-42")
```

### 8.1 Insert

`insert(document)` fails if the document already exists.

### 8.2 Upsert

`upsert(document)` inserts or replaces according to Search Kernel semantics.

Conditional upsert is exposed directly without requiring a builder:

```python
index.upsert(document, condition="inventory_version == 7")
```

### 8.3 Get

```python
doc = index.get("sku-42")
```

Missing documents return the language's ordinary optional/no-value representation rather than throwing `NotFound`.

Projection:

```python
doc = index.get("sku-42", fields=["title", "price"])
```

Presence matters:

```text
fields absent      -> backend/default projection
fields=[]          -> explicitly no fields
fields=[...]       -> requested fields
```

SDKs must preserve that distinction.

### 8.4 Delete

```python
deleted = index.delete("sku-42")
```

The result indicates whether a document was deleted.

Conditional delete:

```python
index.delete("sku-42", condition="inventory_version == 7")
```

Do not use `put` as the primary documented term. An SDK MAY retain `put` as a compatibility alias for `upsert`.

---

## 9. PatchBuilder — simple partial updates

Patch is the intentionally small, easy partial-update API:

```python
(
    index.patch("sku-42")
    .set("price", 129.0)
    .set("in_stock", True)
    .unset("sale_price")
    .execute()
)
```

Supported conceptual operations:

```text
set(field, value)
unset(field)
```

`None`, `null`, `nil`, `false`, and missing map entries MUST NOT implicitly mean unset.

Patch exists for the common case. It is not expected to expose the full Vespa document-update model.

---

## 10. UpdateBuilder — full update power

The advanced update builder exposes the broader portable/Vespa update model without bloating `PatchBuilder`.

Example:

```python
(
    index.update("sku-42")
    .assign("price", 129.0)
    .increment("inventory", 5)
    .add("tags", "featured")
    .remove("tags", "clearance")
    .condition("inventory_version == 7")
    .execute()
)
```

Canonical facade operations:

```text
assign(path, value)
clear(path)
add(path, value)
remove(path, value)
increment(path, value)
decrement(path, value)
multiply(path, value)
divide(path, value)
match(path, element, update)
tensor_modify(path, operation, payload, create=...)
condition(expression)
create(bool)
execute()
```

Field paths may initially be strings using Vespa field-path syntax:

```python
.update("doc-1").increment("people[34].score", 1)
```

A typed field-path helper may be added later without replacing the string form.

Patch and update are intentionally separate:

```text
PatchBuilder  -> simple application ergonomics
UpdateBuilder -> faithful advanced update capability
```

---

# Part III — Search Entry Points

## 11. `search(text)` — primary API

```python
index.search("running shoes")
```

This is the default full-text search entry point.

The text is linguistically analyzed according to Search Kernel/Vespa semantics.

Whitespace-only text SHOULD be rejected. Use `browse()` for match-all workloads.

---

## 12. `browse()` — match all

```python
index.browse()
```

`browse()` means match-all with optional filters, sorting, grouping, projection, and pagination.

Example:

```python
results = (
    index.browse()
    .filter("in_stock = true")
    .sort("price", "asc")
    .facet("brand")
    .limit(50)
    .execute()
)
```

Do not teach users to use `search("")` as browse.

---

## 13. `query(expr)` — advanced typed query

The typed query entry point exposes advanced portable query-tree capability without requiring raw YQL.

```python
results = (
    index.query(
        q.and_(
            q.contains("title", q.phrase("wireless", "headphones")),
            q.range("price", lte=300),
        )
    )
    .ranking("product-search")
    .execute()
)
```

`query(expr)` is for developers who need explicit query-tree semantics.

It is not the recommended replacement for ordinary `search(text)`.

---

## 14. `yql(source)` — raw Vespa query escape hatch

```python
results = (
    index.yql("""
        select * from sources *
        where title contains text(@q)
        order by price asc
    """)
    .property("q", "running shoes")
    .ranking("default")
    .page(1, 20)
    .execute()
)
```

YQL remains Vespa YQL.

Search Kernel MUST NOT pretend that YQL and the high-level API are the same language.

Raw YQL is an explicit expert escape hatch.

---

# Part IV — SearchBuilder

## 15. Conceptual surface

`search()`, `browse()`, and `query()` return the same conceptual configurable search builder, subject to query-source-specific validity rules.

```text
.filter(...)

.search_field(...)
.search_fields(...)

.sort(...)
.sort_relevance(...)
.sort_feature(...)
.raw_sort(...)

.facet(...)
.facets(...)
.aggregate(...)
.group(...)
.raw_grouping(...)

.ranking(...)
.rank_property(...)
.input(...)
.input_expr(...)

.fields(...)
.summary(...)
.highlight(...)

.offset(...)
.limit(...)
.page(...)
.timeout(...)

.language(...)
.locale(...)
.default_index(...)
.sources(...)
.restrict(...)

.property(...)

.execute()
.inspect()
```

Not every advanced method is necessarily supported by every backend. Unsupported backend capability must produce a structured `Unsupported` error rather than being silently ignored.

---

# Part V — Filtering

## 16. Human-readable filters

The primary filter API is a compact expression string:

```python
.filter("price <= 120 AND in_stock = true")
```

Parameterized filters bind their values at the filter call site:

```python
(
    index.search("shoes")
    .filter(
        "price <= :max AND brand IN (:brands)",
        {
            "max": 120,
            "brands": ["Acme", "TrailCo"],
        },
    )
)
```

The filter-expression grammar, typing rules, spans, complexity limits, and binding behavior are defined by the separate **Filter Expression Language Specification**.

The SDK does not implement that grammar independently.

---

## 17. Repeated filters mean AND

```python
(
    index.search("shoes")
    .filter("price <= :max", {"max": 120})
    .filter("in_stock = true")
)
```

means:

```text
(price <= :max) AND (in_stock = true)
```

Each source expression remains distinct for diagnostics.

---

## 18. Filter bindings

Filter parameters are local to the `.filter()` call that contains their placeholders:

```python
.filter(
    "brand = :brand AND category IN (:categories)",
    {
        "brand": "Acme",
        "categories": ["shoes", "boots"],
    },
)
```

The second argument is optional when the expression contains no parameters:

```python
.filter("in_stock = true")
```

Rules:

- bindings are scoped to one filter clause, not to the whole search builder;
- every referenced parameter in that clause MUST be supplied by that clause;
- every supplied binding MUST be referenced by that clause;
- parameter names are case-sensitive;
- values remain typed;
- list-valued bindings remain typed lists;
- the same placeholder name MAY be reused by different filter clauses with different values;
- applications SHOULD prefer bindings over string interpolation.

For example, this is valid and unambiguous:

```python
(
    index.search("shoes")
    .filter("price <= :value", {"value": 120})
    .filter("rating >= :value", {"value": 4})
)
```

Conceptually, each call contributes one source unit:

```text
FilterClause {
    source
    bindings
}
```

SDKs MUST NOT expose builder-global `.param()` or `.params()` methods for filter bindings.

---

## 19. Structured filters remain available

Machine-generated applications may use the public typed filter helper API where useful:

```python
.filter(
    f.and_(
        f.lte("price", 120),
        f.eq("in_stock", True),
    )
)
```

String filters remain the primary human-facing form.

Typed helpers MUST lower into the same canonical filter semantics.

String and structured equivalents must resolve identically.

---

# Part VI — Search Fields

## 20. Search field selection

Common case:

```python
.search_fields("title", "description")
```

Weighted fields:

```python
.search_field("title", weight=3.0)
.search_field("description", weight=1.0)
```

Repeated calls preserve order and configuration.

Duplicate field declarations SHOULD be rejected rather than silently merged.

Search-field selection is valid for the normal text-query source and invalid for query sources where field context is already explicit.

---

# Part VII — Sorting

## 21. Field sorting

```python
.sort("price", "asc")
.sort("rating", "desc")
```

Sort direction is always explicit.

Repeated calls define priority:

```python
.sort("rating", "desc")
.sort("price", "asc")
```

means:

```text
rating DESC, then price ASC
```

---

## 22. Missing-value behavior

The ergonomic form is:

```python
.sort("price", "asc", missing="last")
.sort("price", "asc", missing="first")
.sort("price", "asc", missing=0)
```

These normalize respectively to missing-last, missing-first, and missing-as-value semantics.

---

## 23. Relevance and rank-feature sorting

```python
.sort_relevance("desc")
.sort_feature("freshness", "desc")
```

These remain explicit rather than relying on magic field names such as `_score`.

---

## 24. Raw sort escape hatch

```python
.raw_sort("...")
```

Raw sort source is caller intent only. It must either normalize to canonical resolved sort semantics or be rejected before resolved IR.

Do not silently carry unresolved raw sort text into resolved semantics.

---

# Part VIII — Pagination

## 25. Offset and limit

```python
.offset(40)
.limit(20)
```

Semantics:

```text
offset = zero-based matched hits to skip
limit  = maximum hits returned
```

`limit(0)` is valid for totals/aggregation-only requests.

---

## 26. Page convenience

```python
.page(3, 20)
```

Pages are one-based:

```text
page(1, 20) -> offset 0,  limit 20
page(2, 20) -> offset 20, limit 20
page(3, 20) -> offset 40, limit 20
```

---

## 27. Pagination modes cannot mix

Reject:

```python
.page(2, 20).offset(20)
.page(2, 20).limit(20)
.offset(20).page(2, 20)
.limit(20).page(2, 20)
```

Reject negative values and checked-overflow cases before execution.

---

# Part IX — Facets, Aggregations, and Grouping

## 28. One canonical grouping model

Grouping is the canonical aggregation/faceting mechanism.

Client conveniences such as:

```python
.facet("brand")
.aggregate(agg.avg("price"))
```

must normalize into structured grouping before canonical IR.

There is no independent facet semantic model beneath the facade.

Likewise, returned `facets`, groups, and aggregation conveniences are projections over the canonical recursive result tree.

---

## 29. Facets — simple first

The common hosted-search-style experience remains deliberately simple:

```python
.facet("brand")
.facet("category", limit=20)
```

Multiple fields:

```python
.facets("brand", "category")
```

Useful options:

```python
.facet(
    "brand",
    name="brands",
    limit=20,
    sort="count_desc",
    precision=200,
)
```

Recommended standard facet sort vocabulary:

```text
count_desc
count_asc
value_asc
value_desc
```

The facade translates these into structured grouping order expressions.

A facet is conceptually equivalent to grouping by an attribute and outputting a count.

---

## 30. Root-level metric aggregations

For simple metrics without explicit groups:

```python
results = (
    index.browse()
    .filter("category = 'shoes'")
    .aggregate(agg.count().as_("products"))
    .aggregate(agg.avg("price").as_("avg_price"))
    .aggregate(agg.min("price").as_("min_price"))
    .aggregate(agg.max("price").as_("max_price"))
    .limit(0)
    .execute()
)
```

This is convenience syntax over grouping output, not a separate aggregation engine.

---

## 31. Aggregation helpers

Canonical public aggregation helpers:

```python
agg.count()
agg.sum("price")
agg.avg("price")
agg.min("price")
agg.max("price")
agg.xor("flags")
agg.stddev("price")
agg.quantiles("price", 0.5, 0.9, 0.99)
agg.summary()
agg.summary("compact")
```

Every output may be labeled:

```python
agg.avg("price").as_("avg_price")
```

Labels become stable result-field names.

Aggregation expressions may participate in ordering and arithmetic where the canonical grouping language permits it.

---

## 32. Group — ergonomic structured grouping

For the common advanced case, use `Group.by(...)`:

```python
results = (
    index.search("shoes")
    .group(
        Group.by("brand")
        .limit(10)
        .order_by(desc(agg.count()))
        .output(
            agg.count().as_("products"),
            agg.avg("price").as_("avg_price"),
        )
    )
    .execute()
)
```

`Group.by("brand")` is shorthand for grouping by the attribute expression for `brand`.

An expression may also be supplied:

```python
Group.by(g.fn("time.year", g.field("published_at")))
```

---

## 33. Nested grouping

Nested groups must remain fluent and readable:

```python
results = (
    index.browse()
    .group(
        Group.by("category")
        .limit(20)
        .output(agg.count().as_("products"))
        .subgroup(
            Group.by("brand")
            .limit(10)
            .order_by(desc(agg.count()))
            .output(
                agg.count().as_("products"),
                agg.avg("price").as_("avg_price"),
            )
        )
    )
    .execute()
)
```

The facade normalizes this convenience structure into the canonical `all / group / each / output` grouping tree.

---

## 34. Group limits and precision

```python
Group.by("brand")
    .limit(10)
    .precision(100)
```

`limit()` is the ergonomic facade term for a finite grouping `max`.

For the uncommon explicit infinite case:

```python
Group.by("brand").unlimited()
```

SDK documentation should prefer `limit()` for ordinary use rather than exposing the protocol term `max` as the primary user vocabulary.

---

## 35. Group ordering

```python
.order_by(desc(agg.count()))
.order_by(asc(g.field("brand")))
```

Multiple expressions establish order priority:

```python
.order_by(
    desc(agg.count()),
    asc(g.field("brand")),
)
```

Descending/ascending normalize to canonical unary grouping expressions rather than introducing a second conflicting direction representation.

---

## 36. Group outputs

```python
.output(
    agg.count().as_("count"),
    agg.avg("price").as_("avg_price"),
)
```

Outputs accept grouping expressions, not only aggregations.

Therefore advanced usage may output aliases and computed values:

```python
.output(g.alias_ref("revenue").as_("revenue"))
```

---

## 37. Aliases

```python
Group.by("category")
    .alias("revenue", agg.sum("price"))
    .output(g.alias_ref("revenue").as_("revenue"))
    .order_by(desc(g.alias_ref("revenue")))
```

Aliases are request-local grouping semantics.

Duplicate alias names are errors.

---

## 38. Grouping expressions

The ergonomic grouping expression namespace is `g`.

Primary helpers:

```python
g.field("price")
g.value(100)
g.fn("time.year", g.field("published_at"))
g.alias_ref("revenue")
g.member(expr, "field")
g.lookup(expr, key)
g.bucket(lower=0, upper=100, include_lower=True, include_upper=False)
```

Where a language supports operator overloading cleanly, arithmetic may be natural:

```python
g.field("price") * agg.count()
-agg.count()
```

Languages without suitable operator overloading expose equivalent named helpers.

The semantic representation must be identical.

---

## 39. Grouping functions

Vespa has a large and evolving grouping function family.

Do not create hundreds of one-off SDK methods merely to mirror it.

Use a stable generic helper:

```python
g.fn("time.year", g.field("published_at"))
g.fn("math.sqrt", g.field("score"))
g.fn("fixedwidth", g.field("price"), 10)
```

SDKs MAY add named conveniences for especially common functions later, but `g.fn(...)` remains the stable generic facade.

Function support is validated centrally against the supported Vespa revision.

---

## 40. Group filters and keep

Grouping filtering is distinct from ordinary document filtering.

Ergonomic grouping-filter helpers live under `gf`:

```python
Group.by("brand")
    .filter(gf.range(g.field("price"), 10, 100))
    .keep(gf.regex(g.field("brand"), "^A"))
```

Core helpers:

```python
gf.and_(...)
gf.or_(...)
gf.not_(...)
gf.regex(expr, pattern)
gf.range(expr, lower, upper, include_lower=True, include_upper=False)
gf.is_true(expr)
```

These must not be confused with the top-level `.filter("...")` language.

---

## 41. Streaming grouping `where`

Where supported:

```python
Group.by("brand").where(expression)
```

This maps to streaming-search grouping `where(...)` and remains distinct from normal grouping `filter()` and `keep()`.

Backends that do not support the capability return `Unsupported`.

---

## 42. Summary aggregation

```python
Group.by("brand")
    .limit(10)
    .output(agg.summary("compact").as_("hits"))
```

Summary aggregation returns hits beneath the relevant group according to Vespa grouping semantics.

The result model must preserve these hits as child nodes, not flatten them into top-level hits.

---

## 43. Grouping continuations

Grouping requests may carry continuation tokens:

```python
Group.by("brand").continuation(token)
```

or, for a full request continuation:

```python
.group(grouping.continue_from(token))
```

Exact convenience syntax may vary idiomatically, but continuation tokens remain opaque strings.

Returned group continuations must be preserved without interpretation by clients.

---

## 44. Expert structured grouping

`Group.by(...)` intentionally optimizes the common tree shape.

For arbitrary canonical grouping structure, expose an expert fluent AST builder under `grouping`:

```python
index.browse().group(
    grouping.all(
        grouping.group(g.field("category")),
        grouping.each(
            grouping.output(agg.count().as_("count")),
            grouping.max(20),
        ),
    )
)
```

This API is still a public facade builder. It must not expose generated FlatBuffers classes.

It exists to preserve full structured grouping power including parallel branches and explicit `all` / `each` structure.

---

## 45. Raw grouping escape hatch

```python
.raw_grouping("all(group(category) each(output(count())))")
```

Raw grouping source is lossless caller intent.

It must be parsed/normalized before it can appear as resolved structured semantics. If the implementation cannot normalize it, inspection may return `Unsupported` rather than pretending it was resolved.

Raw grouping is for expert Vespa users. Documentation should prefer structured grouping whenever equivalent.

---

# Part X — Typed Query DSL

## 46. Query namespace

Typed query helpers live under a compact namespace such as `q`.

Examples:

```python
q.contains("title", "rust")
q.contains("title", q.phrase("search", "engine"))
q.and_(...)
q.or_(...)
q.not_(...)
q.eq("category", "books")
q.range("price", gte=10, lt=100)
q.in_("brand", ["Acme", "TrailCo"])
```

Strings supplied as contains-items may be shorthand for `q.term(...)` where unambiguous.

---

## 47. Text/proximity query helpers

Facade coverage should include:

```python
q.term(value)
q.contains(field, item)
q.phrase(*items)
q.near(*items)
q.onear(*items)
q.same_element(*clauses)
q.equiv(*clauses)
q.alternatives(...)
q.fuzzy(value)
q.matches(field, pattern)
q.text(text)
q.user_input(value)
q.user_query()
```

The facade should choose names developers can understand without knowing the generated schema type names.

---

## 48. Weighted and retrieval query helpers

Expose advanced Vespa matching capabilities without raw protocol construction:

```python
q.dot_product(field, items)
q.weighted_set(field, items)
q.wand(field, items)
q.weak_and(*clauses)
q.rank(match, *rank_items)
q.labeled(query, label, score)
q.non_empty(query)
```

Weighted item mappings should accept ordinary host-language maps/dictionaries where ordering semantics do not depend on insertion order.

---

## 49. Geospatial queries

```python
q.geo_location("location", lat=37.3, lon=-121.9, radius="10km")
q.geo_box("location", south=..., west=..., north=..., east=...)
```

Do not invent a separate geospatial query subsystem.

---

## 50. Nearest-neighbor queries

```python
results = (
    index.query(
        q.nearest_neighbor("embedding", "query_embedding")
        .annotate(total_target_hits=100, approximate=True)
    )
    .input("query_embedding", embedding)
    .ranking("semantic")
    .limit(20)
    .execute()
)
```

The query tensor refers to a named request input. It is not duplicated inline inside the nearest-neighbor node.

---

## 51. Predicate queries

```python
q.predicate(
    "targeting",
    features={...},
    range_features={...},
)
```

Predicate feature structures remain typed values and maps.

---

## 52. Query annotations

Query nodes support generic annotations because Vespa's annotation set is broad and evolving.

Generic form:

```python
query.annotate(
    label="candidate",
    weight=150,
    ranked=True,
    filter=False,
    totalTargetHits=100,
)
```

SDKs may provide helpers for common annotations:

```python
query.label("candidate")
query.weight(150)
```

But there must remain one canonical underlying annotation map rather than duplicate representations.

---

# Part XI — Ranking and Query Inputs

## 53. Ranking profile selection

```python
.ranking("product-search")
```

The profile name selects a configured Vespa rank profile.

Search Kernel does not invent a second ranking language in the client.

---

## 54. Rank properties

```python
.rank_property("freshness_weight", "0.25")
.rank_properties({
    "foo": "bar",
})
```

Rank properties are distinct from generic query properties and query inputs.

SDKs must preserve these namespaces separately.

---

## 55. Query inputs

Literal input:

```python
.input("user_embedding", embedding)
.input("category_boost", 1.5)
```

Expression input:

```python
.input_expr("user_embedding", "embed(transformer,@query)")
```

Expression inputs must not be serialized as ordinary string literals.

---

# Part XII — Projection, Summaries, and Highlighting

## 56. Fields

```python
.fields("title", "brand", "price")
```

This controls field projection where valid for the query source.

Presence remains meaningful. SDKs must distinguish no projection from an explicitly empty projection when the protocol does.

---

## 57. Summary selection

```python
.summary("search-result")
```

An empty/default summary and a named summary must remain distinguishable where the backend contract distinguishes them.

---

## 58. Highlighting

```python
.highlight("title", "description")
```

Highlight output remains structured in `Hit.highlights`.

Clients should not inject markup into returned field strings unless an explicitly separate convenience API requests rendered markup.

---

## 59. Presentation controls

Advanced presentation controls may include:

```python
.format("...")
.bolding(True)
.timing(True)
```

These are advanced and should not clutter introductory documentation.

---

# Part XIII — Query Model and Long-Tail Controls

## 60. Common query-model conveniences

Expose common controls directly and fluently:

```python
.language("en")
.locale("en-US")
.default_index("title")
.sources("products")
.restrict("products")
.search_path("...")
.search_group(2)
```

For typed queries using `q.user_query()`:

```python
.query_string("running shoes")
```

---

## 61. Query model type

Advanced Vespa model-type controls should be grouped so they do not flood autocomplete:

```python
.query_model_type(
    composite="...",
    tokenization="...",
    syntax="...",
    profile="...",
    yql_default=True,
)
```

---

## 62. Generic query properties

Long-tail Vespa Query API properties remain available:

```python
.property("some.query.property", value)
.properties({...})
```

Typed facade methods own their canonical property paths.

A generic property that semantically overlaps a populated typed field is an error. There is no precedence rule.

This keeps the facade extensible without making all Vespa Query API properties first-class methods.

---

## 63. Query profile and search chain

Where represented and supported:

```python
.query_profile("profile-name")
.search_chain("chain-name")
```

These controls are container-oriented and may be unsupported by the embedded backend.

The facade may expose them for protocol parity, but backend rejection must be explicit.

---

# Part XIV — YqlBuilder

## 64. YQL property substitution

YQL `@name` substitution uses ordinary YQL query properties rather than filter-style bindings:

```python
(
    index.yql("select * from sources * where title contains text(@q)")
    .property("q", "running shoes")
)
```

These values are request-wide query properties. They are intentionally distinct from filter-local bindings, query inputs, and rank properties.

`YqlBuilder` MUST NOT expose `.param()` merely as an alias for `.property()`.

---

## 65. Valid YQL builder controls

YQL may coexist with request-level controls that do not compete with YQL-owned clauses:

```text
.property(...)
.group(...)
.raw_grouping(...)
.ranking(...)
.rank_property(...)
.input(...)
.input_expr(...)
.offset(...)
.limit(...)
.page(...)
.timeout(...)
.execute()
.inspect()
```

Programmatic Search Kernel grouping is intentionally valid alongside YQL.

This is important: raw YQL grouping syntax is not the only grouping path for a YQL query.

---

## 66. Invalid YQL combinations

The high-level YQL builder does not expose operations that compete directly with YQL query syntax:

```text
filter
sort
search_fields
high-level projection/highlighting where YQL SELECT/output owns them
```

Invalid combinations must fail deterministically rather than being silently merged.

Search Kernel does not parse raw YQL merely to guess whether the source text already contains a particular clause.

---

# Part XV — Timeout and Execution

## 67. Timeout

```python
.timeout(500)
```

The cross-language facade defines integer timeout values in milliseconds.

SDKs MAY additionally accept language-native duration types where unambiguous, but must normalize exactly to milliseconds.

---

## 68. Execute

```python
result = builder.execute()
```

Execution is the only operation that sends the configured request to the target backend.

Builders SHOULD be reusable only if the language's API conventions make that unambiguous. Otherwise, implementations may document them as single-use values.

Mutating a builder after execution SHOULD be rejected if reuse semantics would otherwise be ambiguous.

---

## 69. Inspect without executing

Search builders expose:

```python
resolved = builder.inspect()
```

This invokes Search Kernel query inspection and returns the normalized/resolved public request representation.

Use cases:

```text
What defaults were applied?
How did my filters bind?
What grouping tree was produced by facet/group helpers?
What query inputs and ranking settings will execute?
```

Inspection must show canonical resolved semantics rather than merely echoing caller input.

This is especially useful for proving that ergonomic facade conveniences lower correctly.

---

# Part XVI — Analyze

## 70. AnalyzeBuilder

```python
result = (
    index.analyze("Running with the foxes")
    .field("title")
    .language("en")
    .execute()
)
```

Alternatively, analyze against an explicit analyzer where supported:

```python
index.analyze(text).analyzer(analyzer).execute()
```

Exactly one analysis target is selected.

Result tokens expose:

```text
surface
term
position
start_byte
end_byte
```

The client must not reimplement linguistic analysis.

---

# Part XVII — Search Results

## 71. Canonical result tree

The canonical search result is recursive:

```text
SearchResult
    root: ResultNode
        Hit
        OR
        ResultGroup
            children: ResultNode[]
```

Ordinary search generally contains hit nodes.

Grouping queries may contain nested groups, hits within groups, or mixed group/hit children.

Clients MUST NOT flatten this tree destructively.

---

## 72. SearchResult ergonomic surface

The public result exposes:

```text
root
total_hits
total_hits_relation
coverage
timing
errors
trace
metadata
```

Convenience projections:

```text
hits
groups
facets
aggregations
```

These are views over `root`, not independent semantics.

---

## 73. Hits

A hit exposes:

```text
id
relevance
fields
highlights
source
types
match_features
summary_features
rank_features
sort_values
```

For ergonomics:

```python
hit["title"]
hit.get("price")
```

may proxy into `hit.fields`.

Python `Hit` SHOULD implement the mapping protocol over fields.

JavaScript/TypeScript MAY expose equivalent property conveniences where they do not collide with hit metadata.

Do not describe `fields` as a guaranteed complete stored document: projections and summaries may return only selected result fields.

---

## 74. Score convenience

Where useful, SDKs may expose:

```python
hit.score
```

as an ergonomic alias/view of relevance.

The exact `relevance` representation must remain available because Vespa results can contain positive or negative infinity, which ordinary JSON numbers cannot represent faithfully.

Clients must not silently coerce infinite relevance to null, zero, or a finite sentinel.

---

## 75. Result groups

A `ResultGroup` exposes:

```text
id
label
value
fields
children
continuations
source
types
relevance
limits
metadata
```

Ergonomic projections:

```python
group.groups       # direct ResultGroup children
group.hits         # direct Hit children
group["count"]     # fields lookup
group.get("avg_price")
```

These helpers preserve the original child ordering.

---

## 76. Facet projection

A request such as:

```python
.facet("brand", name="brands")
```

may expose:

```python
result.facets["brands"]
```

with ergonomic buckets:

```text
FacetBucket
    value
    count
```

But this convenience must be derived from the exact structured grouping shape generated by `.facet()`.

Do not guess arbitrary grouping trees into facets.

If a grouping tree does not match the defined facet shape, it remains available through `root/groups` rather than being partially projected as a facet.

---

## 77. Aggregation projection

Labeled root-level aggregations may be exposed as:

```python
result.aggregations["avg_price"]
```

Grouped aggregations remain available on the corresponding group fields:

```python
group["avg_price"]
```

Aggregation conveniences never replace the canonical tree.

---

## 78. Total hits

```python
result.total_hits
result.total_hits_relation
```

Relation may indicate:

```text
exact
lower_bound
approximate
unknown
```

`total_hits` describes the full matched result set, not merely the returned page.

---

## 79. Coverage

Expose coverage as structured data, including degraded reasons and metadata.

Do not collapse coverage into a single percentage if richer fields were returned.

---

## 80. Timing

Timing values are semantically seconds in the portable result.

SDKs may expose host-language duration conveniences, but must not silently reinterpret or rescale the canonical values in serialization.

---

## 81. Search-result errors

Search results may contain backend/result errors separately from operation-level portable errors.

Preserve:

```text
code
summary
message
source
stack_trace
transient
metadata
```

The SDK may offer helpers such as:

```python
result.has_errors
```

but must not discard the structured error list.

---

# Part XVIII — Portable Operation Errors

## 82. Structured exception model

Operation failures become an idiomatic language error/exception while preserving portable structure.

Conceptually:

```text
SearchKernelError
    code
    message
    details[]
    retryable
```

Each detail exposes:

```text
code
path
message
metadata
```

Applications must be able to branch on machine-readable codes without parsing human-readable messages.

Example:

```python
try:
    index.search("shoes").filter("prcie < 100").execute()
except SearchKernelError as error:
    assert error.details[0].code == "unknown_field"
    assert error.details[0].path == "filters[0]"
```

---

# Part XIX — Cross-Language Examples

## 83. Python

```python
results = (
    index.search("running shoes")
    .filter("price <= :max AND in_stock = true", {"max": 120})
    .search_field("title", weight=2)
    .search_field("description", weight=1)
    .sort("price", "asc", missing="last")
    .facet("brand", limit=10)
    .ranking("product-search")
    .fields("title", "brand", "price")
    .page(1, 20)
    .execute()
)
```

---

## 84. TypeScript

```typescript
const results = await index
  .search("running shoes")
  .filter("price <= :max AND in_stock = true", { max: 120 })
  .searchField("title", { weight: 2 })
  .searchField("description", { weight: 1 })
  .sort("price", "asc", { missing: "last" })
  .facet("brand", { limit: 10 })
  .ranking("product-search")
  .fields("title", "brand", "price")
  .page(1, 20)
  .execute();
```

---

## 85. Rust

```rust
let results = index
    .search("running shoes")
    .filter(
        "price <= :max AND in_stock = true",
        params! { "max" => 120.0 },
    )
    .search_field(SearchField::new("title").weight(2.0))
    .search_field(SearchField::new("description").weight(1.0))
    .sort(Sort::field("price").asc().missing_last())
    .facet(Facet::new("brand").limit(10))
    .ranking("product-search")
    .fields(["title", "brand", "price"])
    .page(1, 20)
    .execute()?;
```

Rust may use typed helper values where they improve compile-time safety without changing the conceptual sequence.

---

## 86. Go

```go
results, err := index.
    Search("running shoes").
    Filter("price <= :max AND in_stock = true", searchkernel.Params{
        "max": 120.0,
    }).
    SearchField("title", searchkernel.Weight(2)).
    SearchField("description", searchkernel.Weight(1)).
    Sort("price", searchkernel.Asc, searchkernel.MissingLast()).
    Facet("brand", searchkernel.FacetLimit(10)).
    Ranking("product-search").
    Fields("title", "brand", "price").
    Page(1, 20).
    Execute(ctx)
```

---

## 87. PHP

```php
$results = $index
    ->search("running shoes")
    ->filter("price <= :max AND in_stock = true", ["max" => 120])
    ->searchField("title", weight: 2)
    ->searchField("description", weight: 1)
    ->sort("price", "asc", missing: "last")
    ->facet("brand", limit: 10)
    ->ranking("product-search")
    ->fields("title", "brand", "price")
    ->page(1, 20)
    ->execute();
```

Exact advanced option syntax may be language-idiomatic. Vocabulary and semantics must remain aligned.

---

# Part XX — What the Client Must Not Do

## 88. No protocol-object tax

Normal documentation must not require code resembling:

```python
GroupingOperation(
    operation=GroupingOperationKind(...)
)
```

or:

```python
SearchRequest(
    query=QuerySource(...),
    ...
)
```

Generated types may exist for protocol/tooling users, but they are not the application API.

---

## 89. No giant generic options dictionary as the primary API

Avoid making the normal experience:

```python
index.search("shoes", {
    "filter": ...,
    "sort": ...,
    "grouping": ...,
    "ranking": ...,
    "presentation": ...,
})
```

A generic request object may exist for low-level interoperability, but fluent discoverability is the primary product interface.

---

## 90. No client-specific search semantics

Do not allow Python, TypeScript, Rust, Go, and PHP to grow different primary filter/query/grouping meanings.

Language-specific syntactic sugar is allowed only when it lowers immediately and losslessly into the same portable semantics.

---

## 91. No fake simplification that removes Vespa power

Ergonomics must not mean permanently collapsing capabilities into a small hosted-search subset.

Search Kernel should provide easy shortcuts for:

```text
text search
filters
sorting
facets
pagination
```

while still exposing:

```text
typed query trees
nearest-neighbor retrieval
query inputs
ranking profiles/properties
advanced sorting
structured grouping
nested groups
aggregations
group expressions
continuations
summary aggregation
raw YQL
raw grouping
long-tail query properties
```

---

## 92. No accidental dual semantics

Conveniences must normalize rather than coexist as unrelated parallel models.

Examples:

```text
facet()      -> structured grouping
aggregate()  -> structured grouping
Group.by()   -> structured grouping

search(text) -> SearchKernelTextQuery
browse()     -> MatchAllQuery
query(expr)  -> TypedQuery
yql(source)  -> YqlQuery
```

Results follow the same rule:

```text
hits/facets/aggregations/groups
        ↓ views over
canonical ResultNode tree
```

---

# Part XXI — Documentation Strategy

## 93. Teach the 90% path first

Primary README/tutorial examples should emphasize:

```text
search
filter
sort
facet
page
fields
```

Do not introduce users to `GroupingAllOperation`, query annotations, rank properties, or raw YQL in the first example.

---

## 94. Advanced concepts should feel additive

Recommended documentation progression:

```text
1. Search documents
2. Filter and sort
3. Facets and pagination
4. Ranking and projections
5. Vector / typed queries
6. Grouping and aggregations
7. Query inspection
8. Raw YQL and Vespa escape hatches
```

The user should never need to abandon the client API and start constructing wire objects merely because their query becomes sophisticated.

---

# Part XXII — Conformance and Acceptance

## 95. Cross-language semantic traces

Freeze representative facade traces and assert that each SDK produces equivalent canonical requests.

Example:

```text
Search("running shoes")
Filter("price <= :max", {max: Float(120)})
Sort("price", Asc, MissingLast)
Facet("brand", limit=10)
Ranking("product-search")
Fields("title", "brand", "price")
Page(1, 20)
Execute
```

Targets should include:

```text
Rust
Python
TypeScript
Go
PHP
```

---

## 96. Python real-engine E2E acceptance

The Python client is not considered functional merely because builders serialize.

A real-engine E2E acceptance script must prove at least:

### Documents

```text
insert/upsert/get/delete
patch set/unset
advanced update representative
persistence/reopen where embedded testing applies
```

### Search basics

```text
text search
stemming/linguistics representative
string filters
filter-local bindings
repeated filters
sorting
pagination
projection
total hits
```

### Facets

```text
single facet
multiple facets
filtered facet
facet limit/order
facet result projection
```

### Grouping and aggregation

```text
root aggregation
count
sum/avg/min/max
stddev or quantiles representative
single-level group
nested group
group ordering by aggregation
group limit
group precision
alias/output representative
group filter or keep representative
summary aggregation producing Hit children
recursive result-tree decoding
continuation-token preservation
```

### Query/ranking

```text
typed query representative
query input
ranking profile
rank property representative
nearest-neighbor path when supported
```

### YQL

```text
raw YQL
YQL property substitution
YQL + request pagination
YQL + ranking
YQL + programmatic grouping
invalid YQL/high-level combinations rejected
```

### Results/errors

```text
Hit field access
relevance
features/highlights where available
ResultGroup children
mixed hit/group tree
coverage/timing representative
structured operation error
structured search-result error representative
```

The acceptance path must exercise the public Python facade through the production transport/path intended for users.

---

## 97. Builder misuse tests

Every SDK should reject obvious local misuse consistently:

```text
page + offset
page + limit
negative limit/offset
invalid page number
invalid explicit sort direction
missing/unused filter bindings
duplicate aliases
empty required names
YQL + high-level filter
YQL + high-level sort
```

Semantic validation still belongs centrally.

---

## 98. Grouping conformance

For each grouping convenience, compare the resolved inspected request against the expected canonical grouping tree.

At minimum:

```text
facet(field)
aggregate(count)
Group.by(field)
nested Group.by
order by aggregation
alias + output
summary aggregation
raw grouping behavior
```

This prevents ergonomic sugar from becoming a second grouping implementation.

---

## 99. Result-tree conformance

Freeze recursive result fixtures containing:

```text
direct root hits
single group with hits
nested groups
group fields/aggregations
mixed groups and hits
finite and infinite relevance
group limits
continuations
metadata
```

Every language client must decode them losslessly.

---

# Part XXIII — Final Public Shape

## 100. Summary

The intended public client shape is:

```text
DOCUMENTS

insert(document)
upsert(document, condition=...)
get(id, fields=...)
delete(id, condition=...)

patch(id)
    .set(...)
    .unset(...)
    .execute()

update(id)
    .assign(...)
    .clear(...)
    .add(...)
    .remove(...)
    .increment(...)
    .decrement(...)
    .multiply(...)
    .divide(...)
    .match(...)
    .tensor_modify(...)
    .condition(...)
    .create(...)
    .execute()


SIMPLE SEARCH

search(text)
    .filter(...)
    .search_field(...)
    .sort(...)
    .facet(...)
    .ranking(...)
    .fields(...)
    .summary(...)
    .highlight(...)
    .page(...) / .offset(...).limit(...)
    .execute()


BROWSE

browse()
    [same request controls without text input]


TYPED QUERY

query(q....)
    [same compatible request controls]


GROUPING / AGGREGATIONS

.aggregate(agg....)
.group(Group.by(...))
.group(grouping....)       # expert structured tree
.raw_grouping(source)      # Vespa escape hatch


RAW VESPA

yql(source)
    .property(...)
    .group(...)
    .ranking(...)
    .input(...)
    .page(...) / .offset(...).limit(...)
    .execute()


INTROSPECTION

builder.inspect()
index.analyze(text)...execute()
index.describe()
```

The product goal is deliberate:

> Search Kernel should feel simpler than Vespa until the developer needs Vespa's power. Then the power should be available without forcing them to abandon the fluent client API.

The high-level API is not a reduced search engine. It is a developer-friendly facade over the canonical Search Kernel contract and the underlying Vespa capabilities.

---

# 101. Acceptance criteria

This specification is satisfied when:

1. All configurable public operations use fluent builders.
2. Basic search/filter/sort/facet/page usage is obvious and requires no generated protocol types.
3. `search`, `browse`, `query`, and `yql` are distinct query-source entry points with deterministic combination rules.
4. Filter strings and typed filter helpers lower to the same canonical filter IR.
5. String-filter bindings are local to each `.filter()` call; builders expose no global `.param()` / `.params()` filter state.
6. Facets and root aggregations normalize into canonical structured grouping.
7. `Group.by(...)` supports common nested grouping ergonomically.
8. An expert structured-grouping facade can express the canonical grouping tree without generated FlatBuffers classes.
9. Raw grouping remains available as an explicit Vespa escape hatch.
10. The aggregation surface covers count, sum, avg, min, max, xor, stddev, quantiles, and summary.
11. Group expressions support attributes, literals, functions, arithmetic, aliases, member/map access, buckets, and aggregations as expressions.
12. Typed queries cover the portable query-expression surface without requiring raw YQL.
13. Ranking profiles, rank properties, query inputs, and generic query properties remain distinct namespaces.
14. Projection, summaries, highlighting, pagination, timing, and query-model controls lower losslessly.
15. YQL can coexist with programmatic grouping, ranking, inputs, properties, pagination, and timeout as allowed by the canonical contract.
16. Invalid YQL/high-level combinations are rejected rather than guessed or merged.
17. `SearchResult.root` remains the canonical recursive result tree.
18. Hit/group/facet/aggregation conveniences are non-destructive projections over that tree.
19. Recursive groups containing both hits and groups round-trip through every supported SDK.
20. Structured operation errors and search-result errors are preserved without string parsing.
21. Cross-language conformance proves equivalent facade calls produce equivalent portable requests.
22. A real Python E2E suite proves ordinary search plus advanced grouping/aggregation/query/result functionality through the actual user-facing execution path.
