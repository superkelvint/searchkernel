# Search Kernel Filter Expression Language Specification

**Status:** Proposed specification  
**Audience:** Search Kernel API, IR, filter parser, transport, client, and Vespa adapter implementers  
**Scope:** Structured filter expression syntax and semantics; clause-local parameter binding; schema-aware resolution; portable representation; Rust implementation; conformance and verification

---

## 1. Purpose

Search Kernel exposes a small human-readable expression language for structured filtering.

The language is used by the public client API through calls such as:

```python
results = (
    index.search("running shoes")
    .filter(
        "price <= :max AND in_stock = true",
        {"max": 120},
    )
    .execute()
)
```

This specification defines the meaning of the expression passed to `.filter(...)`.

It does **not** define the rest of the Search Kernel client API.

The public client facade, including:

```text
Index
SearchBuilder
BrowseBuilder
YqlBuilder
sorting
pagination
facets
grouping
aggregations
ranking
result objects
```

is defined by `client-api-spec.md`.

The canonical portable request and response representation remains defined by the FlatBuffers API contract.

This document defines only:

```text
filter source text
        +
clause-local typed bindings
        ↓
parse
        ↓
schema-aware binding
        ↓
typed Filter IR
```

---

## 2. Design principles

### 2.1 Structured filtering is not full-text search

Full-text retrieval and structured filtering are different operations.

```python
index.search("running shoes")
```

performs linguistically analyzed text retrieval.

```python
.filter("brand = 'Acme' AND price <= 120")
```

constrains which documents are eligible.

A filter does not silently become full-text matching.

### 2.2 The language is deliberately small

The filter language is not:

- YQL;
- SQL;
- a full-text query language;
- an Elasticsearch-style JSON query DSL;
- a general-purpose expression language.

It exists to make ordinary structured filtering concise, safe, portable, and predictable.

### 2.3 One parser defines semantics

Non-Rust SDKs MUST NOT independently parse or interpret the filter language.

Clients transport:

```text
source text
typed clause-local bindings
```

Rust performs:

```text
lex
parse
bind
schema resolve
to_ir
```

This guarantees identical syntax and semantics across Python, Rust, TypeScript, PHP, Go, and future SDKs.

### 2.4 Runtime values are bound, not interpolated

Application values should normally be supplied separately from the expression:

```python
.filter("price <= :max", {"max": 120})
```

not:

```python
.filter(f"price <= {max_price}")
```

Bindings preserve runtime types, avoid quoting/escaping problems, and make the expression reusable.

### 2.5 Bindings belong to the filter clause

Bindings are local to the `.filter(...)` call that contains the expression.

Conceptually:

```text
FilterClause {
    source
    bindings
}
```

not:

```text
SearchRequest {
    filters[]
    global_filter_parameters{}
}
```

This locality is normative.

---

# Part I — Language Surface

## 3. Basic examples

### Equality

```text
brand = 'Acme'
in_stock = true
rating = 5
```

### Numeric comparison

```text
price < 100
price <= 100
rating > 3
rating >= 4
```

### Boolean composition

```text
price <= 120 AND in_stock = true
```

### OR

```text
brand = 'Acme' OR brand = 'TrailCo'
```

### Grouping

```text
in_stock = true AND (
    brand = 'Acme'
    OR rating >= 5
)
```

### Negation

```text
NOT (brand = 'Budget')
```

### Membership

```text
brand IN ('Acme', 'TrailCo', 'Contoso')
```

### Scalar binding

```text
price <= :max_price
```

used as:

```python
.filter("price <= :max_price", {"max_price": 120})
```

### Multiple bindings

```text
price <= :max_price AND brand = :brand
```

used as:

```python
.filter(
    "price <= :max_price AND brand = :brand",
    {
        "max_price": 120,
        "brand": "Acme",
    },
)
```

### List binding

```text
brand IN (:brands)
```

used as:

```python
.filter(
    "brand IN (:brands)",
    {"brands": ["Acme", "TrailCo"]},
)
```

---

## 4. v1 grammar

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

## 5. Keywords and identifiers

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

The filter language MUST NOT create a second incompatible field-name grammar. If the public schema permits field names that cannot be represented safely by this grammar, that mismatch must be resolved explicitly before the language is frozen.

Parameters use:

```text
:[A-Za-z_][A-Za-z0-9_]*
```

Parameter names are case-sensitive.

---

## 6. String literals

v1 uses single-quoted strings:

```text
brand = 'Acme'
```

A single quote is escaped by doubling it:

```text
publisher = 'O''Reilly'
```

Runtime application values should normally use bindings instead:

```python
.filter("publisher = :publisher", {"publisher": user_publisher})
```

There is no implicit conversion from strings to numbers or booleans.

```text
'100' != numeric 100
'true' != boolean true
```

---

## 7. Numeric literals

The lexer preserves numeric literal text until schema binding.

Do not eagerly convert all numeric literals to `f64`.

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

# Part II — Clause-Local Bindings

## 8. Client shape

The client-facing operation is conceptually:

```text
filter(expression)
filter(expression, bindings)
```

The second argument is optional.

A filter with no runtime values is valid:

```python
.filter("in_stock = true")
```

A filter with runtime values supplies them on the same call:

```python
.filter("price <= :max", {"max": 120})
```

Search and browse use the same filter semantics.

The client facade MUST NOT require a separate request-global:

```text
.param(...)
.params(...)
```

operation for filter bindings.

Request-wide named values used by unrelated features, such as ranking inputs or YQL properties, are separate concepts and are outside this specification.

---

## 9. Binding scope

Each filter clause has its own binding namespace.

For example:

```python
index.search("shoes") \
    .filter("price <= :value", {"value": 120}) \
    .filter("rating >= :value", {"value": 4})
```

is valid.

The two `:value` references are unrelated because they belong to different filter clauses.

Conceptually:

```text
Clause 0
    source:   price <= :value
    bindings: value -> Float(120)

Clause 1
    source:   rating >= :value
    bindings: value -> Integer(4)
```

There is no collision between those names.

---

## 10. Repeated filters mean AND

Repeated `.filter(...)` calls are combined with logical AND.

```python
index.search("shoes") \
    .filter("price <= :max", {"max": 120}) \
    .filter("in_stock = true")
```

is semantically equivalent to:

```text
(price <= 120) AND (in_stock = true)
```

Each submitted expression remains a distinct source unit.

Implementations MUST NOT concatenate source strings and then reparse them.

This preserves:

- independent binding scopes;
- clause indexes;
- precise diagnostics;
- useful source spans.

The resolved roots are combined under the canonical logical AND / `All` representation.

---

## 11. Missing and unused bindings

Binding validation occurs per clause.

For each clause:

- every referenced parameter MUST be supplied;
- every supplied parameter MUST be referenced;
- a parameter reference resolves only against that clause's bindings.

Example:

```python
.filter("price <= :max", {})
```

fails because `max` is missing.

Example:

```python
.filter(
    "price <= :max",
    {"max": 120, "unused": 4},
)
```

fails because `unused` is not referenced.

This strictness catches misspellings and stale application code.

---

## 12. Duplicate bindings

The semantic binding map cannot contain the same name more than once.

Many host-language map/object literals already make duplicate keys impossible or overwrite them before Search Kernel receives the value.

However, the portable representation and Rust binder MUST still reject duplicate binding names if they are representable at that layer.

Do not silently use the first or last value.

---

## 13. Binding types

Bindings preserve their runtime portable types.

Examples:

```text
Bool
Integer
Float
String
List
```

A client MUST NOT stringify values merely to substitute them into source text.

For example:

```python
.filter("price <= :max", {"max": 120})
```

transports a numeric `120`, not `"120"`.

Schema binding then validates and normalizes it according to the type of `price`.

---

## 14. List bindings

A parameter may supply the complete value list for `IN`:

```text
brand IN (:brands)
```

with:

```python
{"brands": ["Acme", "TrailCo"]}
```

The list is treated as a typed value collection, not textual syntax.

An empty list is an error in v1.

A list binding is valid only where the operator accepts a list.

---

## 15. Cross-language shape

The same semantic operation should remain natural in supported SDKs.

### Python

```python
.filter("price <= :max", {"max": 120})
```

### TypeScript

```typescript
.filter("price <= :max", { max: 120 })
```

### PHP

```php
->filter("price <= :max", ["max" => 120])
```

### Go

```go
Filter(
    "price <= :max",
    searchkernel.Params{"max": 120.0},
)
```

### Rust

Rust may expose a small ergonomic helper or macro for heterogeneous portable values:

```rust
.filter(
    "price <= :max",
    params! {
        "max" => 120.0,
    },
)
```

The exact host-language container syntax may differ.

The semantics do not.

---

# Part III — Type Semantics

## 16. Schema-aware resolution

The parser is schema-independent.

The binder resolves the parsed expression against the index schema.

It is responsible for:

- field existence;
- filterability;
- operator legality;
- literal typing;
- parameter typing;
- numeric normalization;
- `IN` element typing.

Syntax that is structurally valid may still fail during binding.

For example:

```text
price <= true
```

is syntactically valid but fails because `price` is numeric.

---

## 17. Keyword

Supported in v1:

```text
=
IN
```

Keyword equality is exact structured equality according to Search Kernel keyword semantics.

It is not linguistic text matching.

The Vespa adapter chooses the faithful native representation.

---

## 18. Integer

Supported in v1:

```text
=
<
<=
>
>=
IN
```

Bound values and literals must normalize losslessly to the integer type supported by the resolved field.

Fractional values are rejected.

Overflow is rejected.

---

## 19. Float

Supported in v1:

```text
=
<
<=
>
>=
```

Exact floating-point equality has ordinary exact numeric semantics.

`IN` for Float is outside v1 unless separately verified and added to the contract.

Non-finite values are rejected.

---

## 20. Boolean

Supported in v1:

```text
=
```

Other operators are rejected.

Boolean unset/missing behavior follows the verified Search Kernel/Vespa contract rather than SQL null semantics.

---

## 21. Text

Text fields are not structured-equality fields in v1.

Use full-text search for linguistically analyzed retrieval.

If exact structured filtering of text-like content is required, model it as a Keyword field or add an explicit future capability.

---

# Part IV — Boolean and Membership Semantics

## 22. Negation and missing values

v1 deliberately omits `!=`.

Users may write:

```text
NOT (brand = 'Acme')
```

Negation uses Search Kernel/Vespa query-tree semantics.

It does not introduce SQL three-valued `NULL` semantics.

The exact behavior of negation against unset or missing fields MUST be frozen against the Vespa oracle before release.

Search Kernel MUST NOT invent different missing-field behavior in different clients.

`IS NULL`, `IS NOT NULL`, and generic `EXISTS` syntax are outside filter-language v1 unless explicitly added later.

---

## 23. IN semantics

`IN` is a first-class semantic operation:

```text
brand IN ('Acme', 'TrailCo')
```

It MUST NOT be lowered prematurely into:

```text
brand = 'Acme' OR brand = 'TrailCo'
```

if the canonical IR and backend provide a first-class membership representation.

The resolved IR should preserve membership conceptually as:

```text
In {
    field
    values
}
```

The Vespa adapter should use Vespa's native membership capability where compatible.

A list binding may provide the complete set:

```python
.filter(
    "brand IN (:brands)",
    {"brands": ["Acme", "TrailCo"]},
)
```

An empty `IN` list is an error in v1 rather than silently becoming false.

---

# Part V — Errors and Limits

## 24. Error model

Filter errors are structured Search Kernel errors.

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

Clients may render errors idiomatically but MUST preserve structured error data.

---

## 25. Binding errors are clause-local

Because bindings belong to individual filter clauses, binding errors identify the relevant clause.

Example:

```python
.filter("price <= :value", {"value": 120})
.filter("rating >= :minimum", {"value": 4})
```

The second clause fails with an unknown/missing parameter error for `minimum` and an unused parameter error for `value`.

The first clause is unaffected.

---

## 26. Complexity limits

The parser MUST enforce explicit limits against pathological input.

At minimum:

```text
maximum expression byte length
maximum token count
maximum nesting depth
maximum AST node count
maximum IN value count
maximum bindings per clause
```

Exact numeric limits are implementation constants and should be documented.

Exceeding a language complexity limit returns `expression_too_complex`.

The recursive-descent parser MUST never recurse without a checked nesting bound.

---

# Part VI — Portable Contract

## 27. Preserve clause locality

The portable contract MUST preserve the association between each filter source and its bindings.

Conceptually:

```text
FilterClause {
    expression: string
    bindings: [NamedValue]
}
```

A search or browse request then carries:

```text
filters: [FilterClause]
```

This is preferred to a conceptual shape such as:

```text
filters: [FilterSource]
params: [NamedValue]
```

because a request-wide parameter namespace would destroy the clause-local semantics defined by the public client API.

The exact FlatBuffers table names may differ.

The semantic association MUST NOT.

---

## 28. Typed structured filters remain available

The existing structured `FilterExpr` representation remains the canonical machine/low-level representation where required.

Do not delete the typed representation merely because the public facade accepts source text.

The intended high-level path is:

```text
FilterClause
    source
    bindings
        ↓
portable FlatBuffers
        ↓
Rust filter parser
        ↓
syntax AST
        ↓
schema + binding resolution
        ↓
typed Filter IR
        ↓
backend adapter
```

A lower-level structured request may enter after the parsing stage, but it MUST converge on equivalent typed IR.

---

## 29. No client-side semantic parser

Language SDKs may perform trivial shape checks such as:

```text
expression must be a string
bindings must be a supported map/object
binding values must be portable values
```

They MUST NOT independently implement:

```text
operator precedence
field lookup
type coercion
keyword recognition
IN semantics
missing-value semantics
```

Those semantics belong to Rust.

---

# Part VII — Rust Implementation

## 30. Crate placement

Preferred crate:

```text
core/crates/searchkernel-filter
```

Responsibility:

```text
filter source
    -> syntax AST
    -> bound/resolved Filter IR
```

It MUST have no Vespa dependency.

It may depend on the portable value/schema/IR crates required to resolve fields and construct canonical IR.

The Vespa adapter remains responsible only for translating resolved IR to native Vespa query structures.

---

## 31. Module layout

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

## 32. Lexer

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

The lexer MUST NOT perform schema lookup.

---

## 33. Syntax AST

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

## 34. Parser

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

The parser handles syntax and precedence only.

This input is syntactically valid:

```text
price <= true
```

The parser accepts its structure.

The binder later rejects it because the field/value types are incompatible.

---

## 35. Binding and resolution

The binder receives:

```text
syntax AST
typed bindings for this clause
resolved index schema
```

and produces canonical resolved filter IR.

Illustrative interface:

```rust
pub fn to_ir(
    expression: &Expr,
    bindings: &ParameterMap,
    schema: &ResolvedIndexDefinition,
) -> Result<ResolvedFilterExpr, FilterError>;
```

Responsibilities:

- resolve field names;
- verify field filterability;
- validate operator against field kind;
- resolve parameter references against the current clause only;
- reject missing, unused, and duplicate bindings;
- coerce numeric literals/bindings according to field type;
- reject invalid values;
- preserve logical structure;
- preserve `IN` as a first-class operation;
- produce structured errors with source spans.

It does not generate YQL.

---

## 36. Combining multiple clauses

Each clause is parsed and bound independently.

Conceptually:

```text
for each FilterClause:
    parse(source)
    bind(ast, clause.bindings, schema)
        ↓
    clause_ir
```

Then:

```text
0 clauses -> no filter
1 clause  -> clause_ir
N clauses -> All(clause_ir...)
```

This preserves clause-local bindings and diagnostics all the way through semantic resolution.

---

## 37. Adapter to Vespa

The normal path is:

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

The filter parser MUST NOT contain Vespa-specific query-item logic.

---

# Part VIII — Conformance and Verification

## 38. Filter-language golden corpus

Freeze a backend-independent corpus before implementation changes.

Each case contains:

```text
source
clause-local bindings
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
list binding IN
scalar bindings
multiple bindings in one clause
negative numbers
floating-point numbers
repeated filter clauses
same parameter name reused independently across clauses
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
missing binding
unused binding
duplicate binding in portable representation
binding supplied to wrong clause
empty IN
too-deep nesting
too many AST nodes
too many IN values
too many bindings
numeric overflow
non-finite numeric values
```

---

## 39. Structured-filter equivalence

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

The two paths MUST produce equivalent resolved IR.

This is the central semantic verifier.

---

## 40. Clause-local binding conformance

Freeze explicit cases proving binding locality.

Example A:

```text
clause 0:
    price <= :value
    value = Float(120)

clause 1:
    rating >= :value
    value = Integer(4)
```

must succeed.

Example B:

```text
clause 0:
    price <= :max
    max = Float(120)

clause 1:
    rating >= :min
    bindings = {}
```

must fail only for clause 1.

Example C:

```text
clause 0:
    price <= :max
    bindings = {max: 120, extra: 1}
```

must report `unused_parameter` for `extra`.

The verifier MUST prove there is no hidden request-global parameter namespace.

---

## 41. Cross-language filter conformance

Each language facade must encode equivalent filter clauses.

Representative semantic trace:

```text
Search("running shoes")

FilterClause(
    source = "price <= :max",
    bindings = {
        "max": Float(120)
    }
)

FilterClause(
    source = "in_stock = true",
    bindings = {}
)

Execute
```

Required SDK targets should eventually include:

```text
Rust
Python
TypeScript
PHP
Go
```

The exact host-language map syntax may differ.

The portable semantics MUST be identical.

---

## 42. Vespa verification

For supported filter operations, verify resolved/native behavior against Vespa.

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
filter + grouping/facets
filter + pagination
```

The client facade for sort, grouping, facets, and pagination is specified elsewhere.

These cases exist here only to prove that filters compose correctly with the wider search request.

The oracle MUST freeze behavior for unset/missing values, especially under negation.

Search Kernel follows verified Vespa behavior rather than SQL assumptions.

---

# Part IX — Non-Goals and Reserved Features

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
3. they are represented in typed IR;
4. conformance tests are frozen first.

Full-text matching belongs in the search/query API, not this language.

---

## 44. YQL is outside this specification

Raw Vespa YQL is a separate query source.

Its client surface and interaction rules are defined by `client-api-spec.md`.

The filter parser:

- does not parse YQL;
- does not lower filter expressions into YQL;
- does not accept YQL fragments inside `.filter(...)`.

Simple filters and YQL may ultimately produce similar native query structures, but they remain separate input languages.

---

## 45. Grouping and aggregation are outside this specification

Search Kernel's structured grouping and aggregation APIs are not part of the filter expression language.

This document does not define:

```text
group_by
group
aggregate
facet
count
sum
avg
min
max
stddev
quantiles
summary aggregation
group ordering
group continuation
```

Those belong to `client-api-spec.md` and the canonical portable schema.

A grouping operation may itself contain or compose with typed filter IR where the portable contract supports it, but that does not extend this expression grammar.

---

## 46. No client-specific primary filter languages

Python must not grow:

```text
price__lte
```

while TypeScript grows:

```text
$lte
```

while Rust grows a completely separate semantic language.

Language-specific convenience sugar may exist later, but it MUST compile immediately to the same canonical filter semantics and MUST NOT become the only way to express a feature.

The shared expression language remains the primary human-readable structured-filter syntax.

---

# 47. Relationship to other specifications

The specification boundaries are:

```text
client-api-spec.md
    defines:
        public fluent builders
        search / browse / YQL
        sort / page / fields
        ranking / inputs
        facets
        grouping / aggregations
        result model
        client ergonomics

filter-expression-language.md
    defines:
        syntax accepted by .filter(...)
        clause-local bindings
        grammar
        precedence
        typing
        schema binding
        errors
        Filter IR equivalence

api/searchkernel.fbs
    defines:
        canonical portable representation
        request/response wire contract
```

The intended architecture is:

```text
developer-facing fluent client
        ↓
.filter(source, bindings)
        ↓
portable FilterClause
        ↓
one Rust parser/binder
        ↓
typed Filter IR
        ↓
backend adapter
```

This document owns only the middle language step.

---

# 48. Acceptance criteria

This specification is implemented when:

1. `.filter(expression)` works for clauses with no runtime bindings.
2. `.filter(expression, bindings)` carries typed runtime values with the clause that consumes them.
3. Filter bindings are clause-local; there is no request-global filter parameter namespace.
4. Repeated `.filter(...)` calls combine under logical AND while remaining independent source units.
5. The grammar is frozen by a backend-independent golden corpus.
6. Non-Rust clients do not implement their own semantic filter parser.
7. Filter source and clause-local typed bindings are losslessly representable in the portable FlatBuffers contract.
8. The Rust parser produces a syntax AST with precise source spans.
9. Schema/binding resolution produces canonical typed Filter IR.
10. Missing, unused, duplicate, and wrong-clause bindings fail deterministically with structured errors.
11. `IN` remains first-class through IR and maps to a native Vespa membership primitive where supported.
12. Full-text retrieval remains distinct from structured filtering.
13. Equivalent textual and structured filters resolve identically.
14. Real Vespa verification covers supported boolean, comparison, membership, and missing-value semantics.
15. Cross-language filter calls produce semantically identical portable FilterClause values.
16. The same parameter name may be reused independently in separate clauses.
17. No public SDK requires users to construct generated protocol AST types for ordinary filtering.
18. General client API, YQL, grouping, aggregation, pagination, ranking, and result-model semantics remain outside this specification and are defined elsewhere.
