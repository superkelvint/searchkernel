# Search Kernel Client Facade & Filter Expression Language Specification

**Status:** Proposed specification  
**Audience:** Search Kernel client, API, IR, transport, and Vespa adapter implementers  
**Scope:** Cross-language client shape; structured filter expression language; raw YQL escape hatch; Rust implementation and conformance requirements

---

## 1. Purpose

Search Kernel needs a public client API that is:

- pleasant to use in Python, Rust, TypeScript, PHP, Go, and future languages;
- consistent enough that examples translate almost line-for-line across SDKs;
- compact for common search workloads;
- easy to discover through editor autocomplete;
- faithful to Vespa rather than inventing parallel search semantics;
- fully representable through the portable FlatBuffers contract;
- implemented semantically once in Rust rather than independently in every client.

This specification standardizes two things:

1. the **shape of the public client facade**, centered on builders for configurable operations;
2. a small **filter expression language** for structured filtering.

It also defines a first-class **raw YQL escape hatch** for users who need Vespa functionality that the high-level Search Kernel facade does not expose.

The intended user experience is:

```python
results = (
    index.search("running shoes")
    .filter("price <= :max_price AND in_stock = true")
    .param("max_price", 120)
    .sort("price", "asc")
    .facet("brand")
    .facet("category", limit=20)
    .page(1, 20)
    .execute()
)
```

The same operation should have the same shape in other languages.

---

# Part I — Cross-Language Client Facade

## 2. Design principles

### 2.1 Builders are the standard for configurable operations

Operations with composable options return builders:

```text
search(text)  -> SearchBuilder
browse()      -> BrowseBuilder
yql(source)   -> YqlBuilder
patch(id)     -> PatchBuilder
```

Operations that are already complete commands execute directly:

```text
insert(document)
upsert(document)
get(id)
delete(id)
```

Do not require `.execute()` where no meaningful request construction exists.

### 2.2 The public facade is not the portable generated API

Normal users should interact with concepts such as:

```text
Index
SearchBuilder
BrowseBuilder
PatchBuilder
SearchResult
Hit
```

They should not normally need generated protocol types such as:

```text
SearchRequest
FilterExpr
FacetRequest
SortExpr
PatchOperation
```

Generated types remain the protocol layer underneath the facade.

### 2.3 Same semantics, nearly the same vocabulary

SDKs should use the same operation names wherever the host language permits it:

```text
filter
param
sort
facet
offset
limit
page
fields
ranking
execute
```

Language syntax may differ where required by the language.

The contract is semantic, not byte-for-byte source-code syntax.

### 2.4 Common operations should not require AST construction

The primary filter API is not:

```python
Filter.all(
    Filter.lte("price", 120),
    Filter.eq("in_stock", True),
)
```

and is not a nested JSON query DSL.

Users write a compact expression:

```text
price <= 120 AND in_stock = true
```

Search Kernel parses and resolves it centrally in Rust.

### 2.5 Full-text search and structured filtering are separate

```python
index.search("running shoes")
```

is linguistically analyzed, ranked text retrieval.

```python
.filter("brand = 'Acme' AND price <= 120")
```

is structured filtering.

A filter does not silently become full-text matching.

### 2.6 Vespa remains available

The high-level facade must not become a prison.

Users can bypass the Search Kernel high-level query model and provide raw Vespa YQL through:

```text
index.yql(...)
```

YQL is an explicit alternative query source, not something silently mixed into high-level filtering.

---

## 3. Canonical client operations

### 3.1 Direct document operations

The canonical names are:

```text
insert(document)
upsert(document)
get(id)
delete(id)
```

Semantics:

- `insert` fails if the document already exists.
- `upsert` inserts or replaces according to Search Kernel document semantics.
- `get` returns the current document or no document.
- `delete` removes the document and reports the defined delete result.
- `put` MAY exist as a compatibility/convenience alias for `upsert`, but documentation should use `upsert`.

### 3.2 Patch builder

Patch is explicitly operation-based:

```python
index.patch("doc-1") \
    .set("price", 89.0) \
    .set("in_stock", False) \
    .unset("sale_price") \
    .execute()
```

The portable semantic operations are:

```text
Set(field, value)
Unset(field)
```

`null`, `None`, `nil`, missing map entries, and `false` MUST NOT implicitly mean unset.

This avoids cross-language ambiguity.

---

# 4. SearchBuilder

## 4.1 Construction

```text
index.search(text) -> SearchBuilder
```

`text` is user/full-text query input.

An empty or whitespace-only text query SHOULD be rejected by the high-level facade. Use `browse()` for structured match-all/filter workloads.

## 4.2 Standard operations

The conceptual builder surface is:

```text
search(text)
    .filter(expression)
    .param(name, value)
    .params(values)

    .sort(field, direction)
    .facet(field, options...)

    .offset(offset)
    .limit(count)

    OR

    .page(number, size)

    .fields(...)
    .ranking(...)
    .timeout(...)
    .execute()
```

Not every optional operation must ship in the first implementation, but names and semantics should not diverge between SDKs once exposed.

---

## 5. Filter builder behavior

### 5.1 One filter

```python
index.search("shoes") \
    .filter("price <= 120 AND in_stock = true")
```

### 5.2 Repeated filters mean AND

```python
index.search("shoes") \
    .filter("price <= :max") \
    .filter("in_stock = true") \
    .param("max", 120)
```

is semantically identical to:

```text
(price <= :max) AND (in_stock = true)
```

Each submitted expression remains a separate source unit for diagnostics. Implementations should not concatenate source strings and then reparse them.

### 5.3 Filter context is rank-neutral

Filter expressions constrain eligibility.

They do not contribute linguistic relevance or ranking score unless an explicitly documented future feature says otherwise.

---

# 6. Parameters

Runtime values should normally be passed separately:

```python
index.search("shoes") \
    .filter("price <= :max_price AND brand = :brand") \
    .param("max_price", 120) \
    .param("brand", user_brand)
```

This is preferred over interpolation:

```python
# Discouraged
.filter(f"brand = '{user_brand}'")
```

Parameters provide:

- safe separation of syntax and values;
- preservation of runtime value types;
- schema-aware numeric normalization;
- reusable filter expressions;
- consistent behavior across languages.

## 6.1 Parameter namespace

Parameters are scoped to the request builder.

All `.filter()` clauses on the builder share the same parameter namespace.

## 6.2 Duplicate parameters

Adding the same parameter name more than once is an error.

Do not silently overwrite an earlier binding.

## 6.3 Missing and unused parameters

At execution:

- every referenced parameter MUST be supplied;
- every supplied filter parameter MUST be referenced.

Unknown and unused parameters are structured request errors.

---

# 7. Sorting

Sort direction is always explicit.

```python
.sort("price", "asc")
.sort("rating", "desc")
```

There is no implicit ascending default in the public contract.

The semantic type is:

```text
SortSpec {
    field
    direction: ASC | DESC
}
```

Strongly typed languages should expose an enum where appropriate:

```rust
.sort("price", SortDirection::Asc)
```

Repeated calls establish sort priority in call order:

```python
.sort("rating", "desc")
.sort("price", "asc")
```

means:

```text
rating DESC, then price ASC
```

Sort fields must resolve to sortable fields in the schema.

Duplicate sort fields SHOULD be rejected rather than silently accepted.

If no explicit sort is supplied:

- `search()` uses the backend/ranking result order;
- `browse()` does not promise a stable document order.

Applications requiring deterministic pagination should specify an explicit deterministic sort.

---

# 8. Facets

The common case is:

```python
.facet("brand")
.facet("category")
```

Configured facets remain builder operations:

```python
.facet("category", limit=20)
```

Advanced option syntax may be language-idiomatic, but MUST normalize to the same portable facet request.

Facet counts are computed over the matched result set before hit pagination unless the underlying portable facet contract explicitly defines otherwise.

A field must be facetable in the resolved schema.

Duplicate facet result names SHOULD be rejected.

---

# 9. Pagination

Search Kernel exposes two mutually exclusive pagination styles.

## 9.1 Absolute window

```python
.offset(40)
.limit(20)
```

Semantics:

```text
offset = zero-based number of matched hits to skip
limit = maximum number of hits returned
```

`offset` is zero-based and must be non-negative.

`limit` must be non-negative. `limit = 0` is valid and is useful for totals/facets-only requests.

At the Vespa execution boundary:

```text
offset -> offset
limit -> hits
```

## 9.2 Page convenience

```python
.page(3, 20)
```

Public page numbering is **one-based**:

```text
page(1, 20) -> offset 0,  limit 20
page(2, 20) -> offset 20, limit 20
page(3, 20) -> offset 40, limit 20
```

Normalization:

```text
offset = (page - 1) * size
limit = size
```

Requirements:

- `page >= 1`
- `size >= 1`
- overflow must be checked

## 9.3 Pagination modes cannot be mixed

These are valid:

```python
.offset(40).limit(20)
```

```python
.page(3, 20)
```

These are invalid:

```python
.page(3, 20).offset(40)
.page(3, 20).limit(50)
.offset(40).page(3, 20)
```

The builder must reject conflicting pagination modes before request execution.

`total_hits` describes the complete matched result set, not merely the returned page/window.

---

# 10. BrowseBuilder

`browse()` is the structured-query counterpart to `search()`.

```python
results = (
    index.browse()
    .filter("price <= :max AND in_stock = true")
    .param("max", 100)
    .sort("price", "asc")
    .facet("brand")
    .page(1, 50)
    .execute()
)
```

It has no free-text query input.

Its filtering, sorting, faceting, projection, pagination, and result semantics match `SearchBuilder`.

Do not teach users to use an empty full-text search as browse.

---

# 11. Raw YQL escape hatch

## 11.1 Entry point

```text
index.yql(source) -> YqlBuilder
```

Example:

```python
results = (
    index.yql("""
        select * from sources *
        where title contains text(@q)
          and price <= 120
        order by price asc
    """)
    .property("q", "running shoes")
    .page(1, 20)
    .ranking("default")
    .execute()
)
```

YQL means Vespa YQL.

Search Kernel MUST NOT parse YQL into the high-level filter language or pretend the two languages are equivalent.

## 11.2 YQL owns query-tree semantics

`YqlBuilder` does not expose `.filter()`.

The following is intentionally invalid:

```python
index.yql("select ... where ...") \
    .filter("price <= 100")
```

If a caller chooses YQL, the YQL source owns:

- match expression;
- structured filters;
- boolean structure;
- fielded text matching;
- YQL ordering;
- YQL grouping syntax;
- other YQL query-tree features.

This avoids hidden query rewriting.

## 11.3 YQL builder operations

YQL may still use request/execution-level controls:

```text
.property(name, value)
.offset(offset)
.limit(count)
.page(number, size)
.ranking(profile)
.timeout(...)
.execute()
```

`.property()` supplies Vespa query properties, including values referenced by YQL `@name` substitution where supported by Vespa.

## 11.4 Operations intentionally absent from YqlBuilder

The high-level YQL builder should not expose:

```text
filter
sort
facet
fields
```

when those would compete with syntax already owned by raw YQL.

Use YQL itself for `where`, `order by`, projection, and grouping.

## 11.5 Result-model limitation

Raw YQL is an escape hatch for query capabilities, but Search Kernel can only return result structures represented by its public result model.

If a valid YQL feature produces a result shape that Search Kernel does not yet model, the implementation must return a structured unsupported-result error rather than silently discard or reinterpret it.

Adding YQL does not imply that every Vespa grouping/ranking/result feature is already representable.

---

# 12. Cross-language examples

## 12.1 Python

```python
results = (
    index.search("running shoes")
    .filter("price <= :max AND in_stock = true")
    .param("max", 120)
    .sort("price", "asc")
    .facet("brand")
    .facet("category", limit=20)
    .page(1, 20)
    .execute()
)
```

## 12.2 Rust

```rust
let results = index
    .search("running shoes")
    .filter("price <= :max AND in_stock = true")
    .param("max", 120.0)
    .sort("price", SortDirection::Asc)
    .facet("brand")
    .facet(Facet::new("category").limit(20))
    .page(1, 20)
    .execute()?;
```

## 12.3 TypeScript

```typescript
const results = await index
  .search("running shoes")
  .filter("price <= :max AND in_stock = true")
  .param("max", 120)
  .sort("price", "asc")
  .facet("brand")
  .facet("category", { limit: 20 })
  .page(1, 20)
  .execute();
```

## 12.4 PHP

```php
$results = $index
    ->search("running shoes")
    ->filter("price <= :max AND in_stock = true")
    ->param("max", 120)
    ->sort("price", "asc")
    ->facet("brand")
    ->facet("category", limit: 20)
    ->page(1, 20)
    ->execute();
```

## 12.5 Go

```go
results, err := index.
    Search("running shoes").
    Filter("price <= :max AND in_stock = true").
    Param("max", 120.0).
    Sort("price", searchkernel.Asc).
    Facet("brand").
    Facet("category", searchkernel.FacetLimit(20)).
    Page(1, 20).
    Execute(ctx)
```

The exact syntax for advanced option objects can vary by language. The operation sequence and semantics must not.

---

# 13. Search results

The stable conceptual result model is:

```text
SearchResult
    hits
    total_hits
    total_hits_relation
    facets
```

Each hit is:

```text
Hit
    id
    score
    document
```

Language-specific convenience access is allowed:

```python
hit["title"]
```

but stored document fields semantically belong to:

```text
hit.document
```

This leaves room for future hit metadata such as:

```text
highlights
rank_features
summary_features
match metadata
```

without polluting the document namespace.

---

# Part II — Filter Expression Language

## 14. Purpose

The filter expression language is a deliberately small language for structured filtering.

It is:

- backend-neutral at the public API layer;
- schema-aware during Rust resolution;
- rank-neutral;
- transported as source text plus typed parameter values;
- parsed exactly once in Rust;
- lowered into Search Kernel's existing typed filter/query IR;
- then adapted faithfully to Vespa.

It is **not**:

- YQL;
- SQL;
- a full-text query language;
- a JSON AST;
- an Elasticsearch-style query DSL.

---

# 15. Examples

## Equality

```text
brand = 'Acme'
in_stock = true
rating = 5
```

## Numeric comparison

```text
price < 100
price <= 100
rating > 3
rating >= 4
```

## Boolean composition

```text
price <= 120 AND in_stock = true
```

## OR

```text
brand = 'Acme' OR brand = 'TrailCo'
```

## Grouping

```text
in_stock = true AND (
    brand = 'Acme'
    OR rating >= 5
)
```

## Negation

```text
NOT (brand = 'Budget')
```

## Membership

```text
brand IN ('Acme', 'TrailCo', 'Contoso')
```

## Parameters

```text
price <= :max_price
```

```text
brand = :brand
```

```text
brand IN (:brands)
```

with typed request values such as:

```python
params = {
    "max_price": 120,
    "brand": "Acme",
    "brands": ["Acme", "TrailCo"],
}
```

---

# 16. v1 grammar

Normative grammar:

```ebnf
expression
    = or_expression ;

or_expression
    = and_expression
      { OR and_expression } ;

and_expression
    = not_expression
      { AND not_expression } ;

not_expression
    = NOT not_expression
    | primary ;

primary
    = "(" expression ")"
    | predicate ;

predicate
    = field comparison value
    | field IN "(" in_values ")" ;

comparison
    = "="
    | "<"
    | "<="
    | ">"
    | ">=" ;

in_values
    = parameter
    | value { "," value } ;

value
    = string
    | number
    | boolean
    | parameter ;

parameter
    = ":" identifier ;
```

Operator precedence, highest to lowest:

```text
NOT
AND
OR
```

Therefore:

```text
A = 1 OR B = 2 AND C = 3
```

means:

```text
A = 1 OR (B = 2 AND C = 3)
```

Parentheses override precedence.

---

# 17. Keywords and identifiers

Keywords are case-insensitive:

```text
AND
OR
NOT
IN
TRUE
FALSE
```

These are equivalent to lowercase spellings.

Field identifiers are case-sensitive and follow Search Kernel's schema field-name rules.

The filter language must not create a second incompatible field-name grammar. If the existing public schema permits names that cannot be lexed safely, that mismatch must be resolved before the filter grammar is frozen.

Parameters use:

```text
:[A-Za-z_][A-Za-z0-9_]*
```

Parameter names are case-sensitive.

---

# 18. String literals

v1 uses single-quoted strings:

```text
brand = 'Acme'
```

A single quote is escaped by doubling it:

```text
publisher = 'O''Reilly'
```

Runtime values should normally use parameters, so application code rarely needs manual escaping.

There is no implicit conversion from strings to numbers or booleans.

```text
'100' != numeric 100
'true' != boolean true
```

---

# 19. Numeric literals

The lexer preserves numeric literal text until schema binding.

Do not eagerly convert all numbers to `f64`.

Examples:

```text
120
-5
120.5
-0.25
1e6
```

The binder interprets the literal according to the resolved field type.

For a Float field:

```text
price <= 100
```

normalizes to floating-point `100.0`.

For an Integer field:

```text
rating >= 4
```

normalizes to integer `4`.

For an Integer field:

```text
rating >= 4.5
```

is a type error.

Overflow and non-finite values are rejected.

---

# 20. Type semantics

Resolution uses the index schema.

## 20.1 Keyword

Supported in v1:

```text
=
IN
```

Keyword equality is exact structured equality according to Search Kernel keyword semantics.

It is not linguistic text matching.

The Vespa adapter chooses the faithful native representation, such as exact/word term matching or a native `in` item as appropriate.

## 20.2 Integer

Supported:

```text
=
<
<=
>
>=
IN
```

## 20.3 Float

Supported:

```text
=
<
<=
>
>=
```

Exact floating-point equality has ordinary exact numeric semantics.

`IN` for Float is not part of v1 unless separately verified and added to the contract.

## 20.4 Boolean

Supported:

```text
=
```

Other operators are rejected in v1.

## 20.5 Text

Text fields are not structured-equality fields in v1.

Use:

```python
index.search("running shoes")
```

for linguistically analyzed text retrieval.

If exact structured filtering of text-like content is required, model it as a Keyword field or add an explicit future capability.

---

# 21. Negation and missing values

v1 deliberately omits `!=`.

Users may write:

```text
NOT (brand = 'Acme')
```

Negation uses Search Kernel/Vespa query-tree negation semantics.

It does **not** introduce SQL three-valued `NULL` semantics.

The exact behavior of negation against unset/missing fields must be frozen against the Vespa oracle before release. Search Kernel must not invent different missing-field behavior in different clients.

`IS NULL`, `IS NOT NULL`, and generic `EXISTS` syntax are outside v1 unless the backend contract is explicitly verified and standardized.

---

# 22. IN semantics

`IN` is a first-class semantic operation:

```text
brand IN ('Acme', 'TrailCo')
```

It must not be lowered prematurely into:

```text
brand = 'Acme' OR brand = 'TrailCo'
```

if the backend has an efficient native membership primitive.

The resolved IR should preserve membership:

```text
In {
    field
    values
}
```

The Vespa adapter should use Vespa's native `in` query capability where compatible.

A parameter may provide the entire list:

```text
brand IN (:brands)
```

where `brands` is a typed list.

An empty `IN` list is an error in v1 rather than silently becoming `false`.

---

# 23. Multiple filter clauses

Repeated builder calls:

```python
.filter("A = :a")
.filter("B = :b OR C = :c")
```

normalize to:

```text
(A = :a) AND (B = :b OR C = :c)
```

Each clause is parsed separately.

The resolved IR then combines clause roots under `All` / logical AND.

This preserves useful source spans for diagnostics.

---

# 24. Error model

Filter errors are structured protocol errors.

Stable categories should include at least:

```text
syntax_error
unknown_field
field_not_filterable
unsupported_operator
type_mismatch
unknown_parameter
unused_parameter
duplicate_parameter
invalid_literal
empty_in
expression_too_complex
```

Each source-related error should carry:

```text
message
clause_index
start_offset
end_offset
```

Example rendering:

```text
price <= :max AND
                 ^
expected filter expression
```

Type error:

```text
price = 'cheap'
        ^^^^^^^
expected a numeric value for Float field 'price'
```

Unknown field:

```text
prcie <= 100
^^^^^
unknown field 'prcie'
```

Clients should render these errors idiomatically but preserve the structured details.

---

# 25. Complexity limits

The parser must enforce explicit limits against pathological request input.

At minimum:

```text
maximum expression byte length
maximum token count
maximum nesting depth
maximum AST node count
maximum IN value count
maximum parameter count
```

Exact numeric limits are implementation constants and should be documented.

Exceeding a limit returns `expression_too_complex`.

The recursive-descent parser must never recurse without a checked nesting bound.

---

# Part III — Portable Contract

## 26. One parser, not one parser per SDK

Non-Rust SDKs MUST NOT independently implement the filter grammar.

All clients transport:

```text
filter source
typed parameters
```

through the portable FlatBuffers request.

Rust performs:

```text
parse
bind
schema resolve
to_ir
```

This guarantees identical syntax and semantics across clients.

Client-side validation may check trivial argument shape, but it must not become a second semantic parser.

---

# 27. Conceptual portable model

The exact FlatBuffers schema should reuse existing portable types wherever possible.

Conceptually:

```text
FilterSource {
    expression: string
}

NamedValue {
    name: string
    value: Value
}
```

A high-level search/browse request carries:

```text
filters: [FilterSource]
params: [NamedValue]
sorts: [SortSpec]
facets: [FacetSpec]
pagination: ...
```

The existing structured `FilterExpr` representation should remain available as the machine/low-level representation.

Do not delete the typed filter representation merely because the facade now accepts text.

The intended flow is:

```text
client filter source
        ↓
portable FlatBuffers
        ↓
Rust filter parser
        ↓
syntax AST
        ↓
schema binding / parameter binding
        ↓
existing resolved Filter IR
        ↓
Vespa adapter
```

---

# 28. Query-source separation

High-level Search/Browse and raw YQL must be structurally distinguishable.

Conceptually:

```text
QuerySource =
    TextSearch
    | Browse
    | Yql
```

The implementation may reuse existing operation/request unions instead of introducing this exact type, but invalid combinations must be impossible or rejected deterministically.

Examples of invalid mixed sources:

```text
text search + raw YQL
high-level filter + raw YQL
browse + text search payload
```

Reuse the existing M7 YQL representation and parser/compiler where possible. Do not create a second parallel YQL implementation.

---

# Part IV — Rust Implementation

## 29. Crate placement

Preferred new crate:

```text
core/crates/searchkernel-filter
```

Responsibility:

```text
filter source
    -> syntax AST
    -> bound/resolved filter IR
```

It must have no Vespa dependency.

It may depend on the portable/value/schema/IR crates required to resolve fields and construct existing IR.

The Vespa adapter remains responsible only for translating resolved IR to Vespa-native query structures.

---

# 30. Module layout

Suggested layout:

```text
searchkernel-filter/
    src/
        lib.rs
        lexer.rs
        parser.rs
        ast.rs
        bind.rs
        error.rs
        limits.rs
```

Keep parsing, syntax AST, and schema binding separate.

---

# 31. Lexer

The lexer is small enough to implement manually.

Required token classes:

```text
Identifier
StringLiteral
NumberLiteral
Parameter

Eq
Lt
Lte
Gt
Gte

And
Or
Not
In

True
False

LeftParen
RightParen
Comma
End
```

Every token carries a byte span:

```rust
struct Span {
    start: usize,
    end: usize,
}
```

Keywords are recognized case-insensitively.

Identifiers and parameter names preserve original case.

The lexer must not perform schema lookup.

---

# 32. Syntax AST

The parser produces syntax, not final semantic IR.

Illustrative Rust shape:

```rust
pub enum Expr {
    Predicate(Predicate),
    And(Vec<Spanned<Expr>>),
    Or(Vec<Spanned<Expr>>),
    Not(Box<Spanned<Expr>>),
}

pub enum Predicate {
    Compare {
        field: Spanned<FieldName>,
        op: CompareOp,
        value: Spanned<ValueExpr>,
    },
    In {
        field: Spanned<FieldName>,
        values: InValues,
    },
}

pub enum CompareOp {
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
}

pub enum InValues {
    Values(Vec<Spanned<ValueExpr>>),
    Parameter(Spanned<ParameterName>),
}

pub enum ValueExpr {
    String(String),
    Number(NumericLiteral),
    Boolean(bool),
    Parameter(ParameterName),
}

pub struct NumericLiteral {
    source: String,
}
```

Exact type names may follow repository conventions.

The important invariant is that numeric semantics are not decided prematurely in the lexer/parser.

---

# 33. Parser

Use bounded recursive descent.

Structure:

```text
parse_expression
    -> parse_or
        -> parse_and
            -> parse_not
                -> parse_primary
                    -> parse_predicate
```

The parser handles only syntax and precedence.

This input is syntactically valid:

```text
price <= true
```

The parser should accept its structure.

The binder later rejects it because the field/value types are incompatible.

This separation keeps the parser deterministic and schema-independent.

---

# 34. Binding and resolution

The binder receives:

```text
syntax AST
typed request parameters
resolved index schema
```

and produces the existing resolved filter IR.

Illustrative interface:

```rust
pub fn to_ir(
    expression: &Expr,
    params: &ParameterMap,
    schema: &ResolvedIndexDefinition,
) -> Result<ResolvedFilterExpr, FilterError>;
```

Responsibilities:

- resolve field names;
- verify field filterability;
- validate operator against field kind;
- resolve parameter references;
- coerce numeric literals/parameters according to field type;
- reject invalid values;
- preserve logical structure;
- preserve `IN` as a first-class operation;
- produce structured errors with source spans.

It does not generate YQL.

---

# 35. Adapter to Vespa

The normal high-level path is:

```text
Search Kernel filter expression
        ↓
searchkernel-filter
        ↓
resolved Filter IR
        ↓
searchkernel-vespa adapter
        ↓
native Vespa query representation
```

Examples:

```text
Float field:
price <= 120
    -> numeric range/comparison item
```

```text
Boolean field:
in_stock = true
    -> boolean equality item
```

```text
Keyword field:
brand = 'Acme'
    -> exact keyword filter semantics
```

```text
Keyword membership:
brand IN ('Acme', 'TrailCo')
    -> native Vespa membership/in item
```

The adapter chooses the faithful Vespa representation.

The filter parser must not contain Vespa-specific query-item logic.

---

# 36. Raw YQL implementation path

Raw YQL follows a separate path:

```text
YqlBuilder
    ↓
portable YQL request/source
    ↓
existing YQL compiler/parser path
    ↓
Vespa native query representation
```

Do not lower YQL into the Search Kernel filter AST.

Do not rewrite a Search Kernel filter into YQL as the runtime implementation.

YQL and the high-level filter language may produce equivalent native trees in simple cases, but they remain separate input languages.

---

# Part V — Conformance and Verification

## 37. Filter-language golden corpus

Freeze a backend-independent corpus before implementation.

Each case contains:

```text
source
parameters
schema
expected syntax AST or expected resolved IR
expected error, if invalid
```

Required positive cases include:

```text
simple equality
numeric comparisons
boolean equality
AND precedence
OR precedence
parentheses
nested NOT
keyword IN
integer IN
list parameter IN
scalar parameters
negative numbers
floating-point numbers
repeated builder filter clauses
```

Required negative cases include:

```text
malformed operators
unclosed strings
unclosed parentheses
dangling AND/OR
unknown field
non-filterable field
invalid operator for field type
float literal against integer field
string against numeric field
missing parameter
unused parameter
duplicate parameter
empty IN
too-deep nesting
too many AST nodes
too many IN values
numeric overflow
non-finite numeric values
```

---

# 38. Structured-filter equivalence verifier

For every filter-language feature, create an equivalent low-level structured filter request.

Example:

```text
text filter:
    price <= 120 AND in_stock = true
```

must resolve identically to:

```text
structured IR:
    And(
        Lte(price, Float64(120.0)),
        Eq(in_stock, Bool(true))
    )
```

The two paths must produce identical resolved IR.

This is the central semantic verifier.

---

# 39. Vespa oracle verification

For supported filter operations, verify the resolved/native behavior against Vespa.

At minimum:

```text
numeric equality/range
boolean equality
keyword equality
IN membership
AND
OR
NOT
parenthesized combinations
filter + ranked text query
filter + sort
filter + facets
filter + pagination
```

The oracle must freeze behavior for unset/missing values, especially under negation.

Search Kernel must follow the verified Vespa behavior rather than SQL assumptions.

---

# 40. Cross-language builder conformance

Freeze semantic builder traces such as:

```text
Search("running shoes")
Filter("price <= :max")
Param("max", Float(120))
Filter("in_stock = true")
Sort("price", Asc)
Facet("brand")
Page(1, 20)
Execute
```

Each language facade must produce the same portable request semantics.

Required SDK conformance targets should eventually include:

```text
Rust
Python
TypeScript
PHP
Go
```

Initial rollout may cover fewer languages, but the portable contract and verifier corpus are authoritative from the beginning.

---

# 41. Pagination conformance

Verify exactly:

```text
page(1, 20) == offset(0).limit(20)
page(2, 20) == offset(20).limit(20)
page(3, 20) == offset(40).limit(20)
```

Verify invalid combinations are rejected:

```text
page + offset
page + limit
page < 1
size < 1
offset < 0
limit < 0
overflow
```

---

# 42. YQL conformance

YQL tests must prove:

- raw YQL reaches the existing YQL/native path;
- Search Kernel does not rewrite it through the filter parser;
- `@name` query properties are forwarded correctly;
- offset/limit/page execution parameters work;
- ranking selection works;
- high-level `.filter()` is unavailable on YQL builders;
- unsupported YQL result shapes fail explicitly rather than being silently discarded.

---

# Part VI — Non-Goals and Reserved Features

## 43. Not in filter language v1

Do not add these merely because they are familiar from SQL or other search systems:

```text
!=
BETWEEN
LIKE
MATCH
CONTAINS
STARTS WITH
REGEX
IS NULL
IS NOT NULL
generic functions
arithmetic expressions
field-to-field comparison
subqueries
```

They may be added only when:

1. Search Kernel has a clear semantic contract for them;
2. the Vespa mapping is verified;
3. they are represented in the typed IR;
4. conformance tests are frozen first.

Full-text matching belongs in `search()` or raw YQL.

---

# 44. No client-specific filter languages

Python must not grow:

```text
price__lte
```

while JavaScript grows:

```text
$lte
```

while Rust grows:

```text
Filter::lte
```

as independent primary semantics.

Language-specific convenience sugar may exist later, but it must compile immediately to the same portable contract and must never become the only way to express a feature.

The shared filter expression is the primary human-readable structured-filter syntax.

---

# 45. No generated handwritten semantic mappings

The generated BFBS model remains the protocol foundation.

The client facade may normalize ergonomic builder calls into generated portable types, but it must not reproduce backend/query semantics independently.

Protocol semantics remain generated/centralized.

Product ergonomics remain handwritten and small.

---

# 46. Vespa compatibility basis

This design intentionally follows Vespa's existing model rather than hiding it:

- Vespa YQL separates structured query logic from unstructured text input.
- YQL supports nested boolean combinations and numeric comparisons.
- Vespa has a native `in` query operator for suitable numeric/string fields.
- YQL supports parameter/query-property substitution with `@name` for supported operators.
- Vespa exposes `hits` and `offset` as native execution parameters.
- Ordering can be expressed directly in YQL.
- Grouping can be expressed in YQL after `|`.

Relevant official documentation:

- https://docs.vespa.ai/en/reference/querying/yql.html
- https://docs.vespa.ai/en/reference/api/query.html
- https://docs.vespa.ai/en/querying/query-api.html
- https://docs.vespa.ai/en/querying/query-language.html

Search Kernel's high-level filter syntax remains its own small language. Compatibility is semantic through typed IR and the Vespa adapter, not textual YQL rewriting.

---

# 47. Final API shape

The intended public surface is:

```text
DOCUMENTS

insert(document)
upsert(document)
get(id)
delete(id)


PATCH

patch(id)
    .set(field, value)
    .unset(field)
    .execute()


HIGH-LEVEL SEARCH

search(text)
    .filter(expression)
    .param(name, value)
    .sort(field, ASC|DESC)
    .facet(field, options...)
    .offset(offset)
    .limit(count)

    OR

    .page(number, size)

    .fields(...)
    .ranking(...)
    .timeout(...)
    .execute()


BROWSE

browse()
    .filter(expression)
    .param(name, value)
    .sort(field, ASC|DESC)
    .facet(field, options...)
    .offset(offset)
    .limit(count)

    OR

    .page(number, size)

    .fields(...)
    .execute()


RAW VESPA

yql(source)
    .property(name, value)
    .offset(offset)
    .limit(count)

    OR

    .page(number, size)

    .ranking(...)
    .timeout(...)
    .execute()
```

The normal user should be able to remember the API after seeing it once:

```python
results = (
    index.search("running shoes")
    .filter("price <= :max AND in_stock = true")
    .param("max", 120)
    .sort("price", "asc")
    .facet("brand")
    .page(1, 20)
    .execute()
)
```

When the high-level API is insufficient:

```python
results = (
    index.yql("""
        select * from sources *
        where title contains text(@q)
          and category in ('shoes', 'boots')
        order by price asc
    """)
    .property("q", "running")
    .page(1, 20)
    .execute()
)
```

This is the intended boundary:

```text
simple, memorable Search Kernel facade
                ↓
portable FlatBuffers
                ↓
one Rust semantic implementation
                ↓
typed IR
                ↓
faithful Vespa adapter

        OR, explicitly:

raw Vespa YQL
                ↓
existing YQL path
                ↓
Vespa
```

---

# 48. Acceptance criteria

This specification is implemented when:

1. Search, browse, patch, and YQL builders exist with the semantics defined above.
2. Sort direction is explicit.
3. `offset/limit` and one-based `page(number, size)` both work and cannot be mixed.
4. The filter expression grammar is frozen by a backend-independent golden corpus.
5. Non-Rust clients do not implement their own filter parser.
6. Filter source and typed parameters are losslessly representable in FlatBuffers.
7. The Rust parser produces a syntax AST with precise source spans.
8. Schema/parameter binding produces the existing typed filter IR.
9. `IN` remains first-class through IR and maps to a native Vespa membership primitive where supported.
10. Full-text search remains distinct from structured filters.
11. Raw YQL is a separate query source and reuses the existing YQL implementation.
12. YQL query properties can be passed without string interpolation.
13. Structured errors cross the native/HTTP client boundary without string matching.
14. Equivalent textual and structured filters resolve identically.
15. Real Vespa verification covers supported boolean/comparison/membership semantics.
16. Cross-language facade traces produce semantically identical portable requests.
17. No public SDK requires users to construct generated protocol AST types for normal search usage.
