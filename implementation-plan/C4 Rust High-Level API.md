# C4 — Rust High-Level API

Implement the product-facing Rust API in:

```text
core/crates/searchkernel/
```

M0–C3 are complete. Do not regress them.

## Goal

Provide the idiomatic Rust interface applications use.

The architecture is:

```text
Rust product API
        ↓
FlatBuffers portable API
        ↓
*_to_ir
        ↓
Search Kernel IR
        ↓
Backend
```

C4 owns ergonomics.

It does **not** own:

```text
defaults
schema semantics
type checking
validation
normalization
search execution
Vespa translation
```

Those remain owned by `searchkernel-ir` or the backend.

---

# 1. Dependency boundary

Allowed:

```text
searchkernel
    ↓
searchkernel-ir
    ↓
searchkernel-protocol
```

For tests:

```text
searchkernel
    ↓ dev-dependency
searchkernel-testkit
```

Forbidden:

```text
searchkernel
    X
searchkernel-vespa-engine

searchkernel
    X
searchkernel-vespa-bindings

searchkernel
    X
searchkernel-ir-adapter
```

C4 must build and test with no native SDK and no Vespa installation.

---

# 2. Public vocabulary

The primary public surface is:

```text
Index
IndexDefinition
Field
Analyzer
SearchField
DocumentId

Patch

Filter
FieldRef

Ranking
Sort
Facet

SearchResult<T>
Hit<T>
```

Supporting result types may also be public where necessary:

```text
TotalHits
TotalHitsRelation
FacetResult
FacetValue
HighlightResult
HighlightFragment
HighlightSpan
```

Do not expose:

```text
FlatBuffers generated types
*Ir types as product API
Backend implementation details
Vespa types
```

---

# 3. `Index`

`Index` is the product-facing handle.

Internally it owns:

```text
IndexContext
Arc<dyn Backend>
```

Conceptually:

```rust
pub struct Index {
    context: Arc<IndexContext>,
    backend: Arc<dyn Backend>,
}
```

`Index` should be cheaply cloneable.

If the backend satisfies the existing thread-safety contract, `Index` must also be:

```text
Send
Sync
```

C4 must never fabricate IR directly.

Every operation flows through the real `IndexContext::*_to_ir` functions.

---

# 4. Lifecycle boundary

C4 does **not** implement real filesystem/native lifecycle.

The final integrated API is intended to feel like:

```rust
let index = Index::open("./products")?;
```

but actual path-based:

```text
create
open
close/native lifecycle
```

is implemented in I2 when the product API is connected to the Vespa backend.

Do not implement a fake `Index::open()` that pretends to open an index.

For standalone C4 verification, provide an infrastructure constructor available to tests/testkit, for example:

```rust
Index::from_backend(
    definition,
    Arc<dyn Backend>,
)
```

Prefer gating/document-hiding this constructor so it does not become the primary product API.

It must:

```text
IndexDefinition
    ↓ portable FlatBuffers definition
    ↓ IndexContext::from_api / index_definition_to_ir
    ↓
Index
```

Real `Index::open()` is wired later without changing CRUD/search API semantics.

---

# 5. `IndexDefinition`

Provide an ergonomic schema builder.

Example:

```rust
let definition = IndexDefinition::new()
    .name("products")
    .field(Field::text("title"))
    .field(Field::keyword("brand"))
    .field(Field::float64("price"))
    .field(Field::bool("in_stock"))
    .default_search_fields([
        SearchField::new("title").weight(2.0),
    ]);
```

Required field constructors:

```rust
Field::text(name)
Field::keyword(name)
Field::int64(name)
Field::float64(name)
Field::bool(name)
```

Required field modifiers:

```rust
.multi()

.stored(bool)
.searchable(bool)
.filterable(bool)
.sortable(bool)
.facetable(bool)

.analyzer(Analyzer)
```

These setters represent explicit caller intent.

Critically:

```rust
Field::text("title")
```

must **not** fill Search Kernel defaults in C4.

It produces a portable field with omitted optional capabilities/analyzer/cardinality where appropriate.

`searchkernel-ir` remains the sole owner of defaults and validation.

---

# 6. `Analyzer`

Expose only analyzers Search Kernel actually supports.

Initial API:

```rust
Analyzer::StandardV1
```

or an equivalent constructor:

```rust
Analyzer::standard_v1()
```

Do not expose arbitrary analyzer names/options merely because the FlatBuffers contract can represent them.

Adding a new analyzer is an explicit product API addition.

---

# 7. `SearchField`

Provide:

```rust
SearchField::new("title")
SearchField::new("title").weight(2.0)
```

An omitted weight remains omitted.

Explicit:

```rust
.weight(0.0)
```

must remain explicit zero.

Do not replace it with the default weight in C4.

---

# 8. Documents

Mutation methods accept ordinary serializable Rust values:

```rust
pub fn insert<T: Serialize>(...)
pub fn upsert<T: Serialize>(...)
```

Example:

```rust
index.upsert("sku-42", &product)?;
```

The supplied ID becomes the portable document ID.

The serialized object supplies user fields.

Document serialization is mechanical:

```text
Rust object
    ↓ serde
portable Value representation
    ↓ *_to_ir
validated DocumentIr
```

C4 must not independently enforce schema types.

If serialization produces:

```text
wrong scalar type
null
nested object
unsupported integer representation
invalid multi-value structure
```

represent it faithfully where possible and allow `searchkernel-ir` to return the canonical semantic error.

The top-level document must serialize as an object/map.

Do not infer `_id` from the Rust value.

The ID argument is authoritative.

---

# 9. `DocumentId`

Provide an ergonomic owned identifier type:

```rust
DocumentId
```

Methods accepting IDs should accept:

```rust
impl Into<DocumentId>
```

so normal usage remains:

```rust
index.get::<Product>("sku-42")?;
index.delete("sku-42")?;
```

Do not reinterpret or normalize IDs.

Canonical validity remains owned by `searchkernel-ir`.

---

# 10. Mutation API

Provide:

```rust
index.insert(id, &document)
index.upsert(id, &document)
index.patch(id, patch)
index.delete(id)
index.get::<T>(id)
```

Recommended ergonomic results:

```text
insert  -> Result<()>
upsert  -> Result<()>
patch   -> Result<()>
delete  -> Result<bool>
get     -> Result<Option<T>>
```

The implementation must still consume the actual backend result and verify/map it correctly.

Do not create different mutation semantics in C4.

---

# 11. `Patch`

Provide an ordered patch builder:

```rust
let patch = Patch::new()
    .set("price", 449.0)
    .set("in_stock", true)
    .unset("description");

index.patch("sku-42", patch)?;
```

Required operations:

```text
set
unset
```

Preserve insertion order.

Do not:

```text
deduplicate fields
validate field existence
validate value types
reject duplicate operations
```

in the facade.

Those are core semantics.

C4 only constructs the portable patch representation.

---

# 12. `Filter`

Provide an idiomatic field DSL:

```rust
field("in_stock").eq(true)
field("price").lte(500.0)
field("brand").ne("Acme")
field("price").gt(100.0)
field("price").gte(100.0)
field("price").lt(500.0)
field("price").exists()
```

Support:

```text
eq
ne
lt
lte
gt
gte
range
exists
```

Also support explicit boolean composition:

```rust
Filter::all([...])
Filter::any([...])
Filter::not(filter)
```

or equivalent idiomatic constructors/operators.

No field capability/type validation occurs here.

---

# 13. Repeated `.filter()` semantics

This is a C4 ergonomic rule and must be frozen.

Given:

```rust
index
    .search("headphones")
    .filter(a)
    .filter(b)
    .filter(c)
```

produce the portable equivalent of:

```text
AND(a, b, c)
```

in call order.

Rules:

```text
zero .filter() calls
    -> no filter

one .filter(a)
    -> a

multiple
    -> AND in insertion order
```

Do not perform additional normalization.

`filter_to_ir` owns canonical boolean normalization.

---

# 14. Search builder

`Index::search(query)` returns a fluent search builder.

Required surface:

```rust
index.search(query)

    .search_fields(...)
    .filter(...)
    .sort(...)
    .facet(...)
    .facets(...)

    .ranking(...)

    .projection(...)
    .highlight(...)

    .language(...)

    .limit(...)
    .offset(...)

    .execute::<T>()
```

The exact builder type may be publicly named `Search` or `SearchBuilder`.

Do not require users to construct a portable `SearchRequest`.

---

# 15. Search-field presence

Builder state must preserve the portable distinction.

Never calling:

```rust
.search_fields(...)
```

means:

```text
search_fields absent
```

Calling:

```rust
.search_fields([])
```

means:

```text
search_fields present-empty
```

Calling with values means present-non-empty.

Do not apply index defaults in the builder.

---

# 16. Projection/highlighting presence

Same rule:

```text
no .projection(...)
    -> projection absent

.projection([])
    -> explicit empty projection

.projection([...])
    -> explicit fields
```

and:

```text
no .highlight(...)
    -> highlighting absent

.highlight([])
    -> explicit highlighting request with zero fields

.highlight([...])
    -> requested fields
```

C4 must preserve these distinctions into FlatBuffers.

---

# 17. Limit/offset and optional scalars

Builder omission remains omission.

Example:

```text
no .limit()
    -> limit absent

.limit(0)
    -> explicit zero

.limit(20)
    -> explicit 20
```

Same principle applies wherever the portable contract distinguishes absent from explicit default values.

C4 must never use ordinary Rust defaults in a way that destroys presence.

---

# 18. `Sort`

Provide explicit constructors such as:

```rust
Sort::asc("price")
Sort::desc("price")
```

and:

```rust
.missing_first()
.missing_last()
```

Preserve sort call order.

Do not validate sortability in C4.

---

# 19. `Facet`

Support the concise form:

```rust
.facets(["brand", "category"])
```

where each string becomes a default `Facet`.

Also support explicit facet configuration:

```rust
.facet(
    Facet::new("brand")
        .limit(20)
        .sort(FacetSort::CountDesc)
)
```

Omitted facet limit/sort remain omitted so the core owns defaults.

Explicit:

```rust
.limit(0)
```

must remain zero.

---

# 20. `Ranking`

Initial public variants:

```rust
Ranking::DefaultRelevanceV1
Ranking::Bm25
```

No `.ranking(...)` call means the portable ranking field is absent.

An explicit `DefaultRelevanceV1` remains explicit.

Do not expose Vespa ranking profiles or expressions through this type.

---

# 21. Execution path

Every operation follows this sequence.

For search:

```text
Search builder
    ↓
FlatBuffers SearchRequest
    ↓
IndexContext::search_to_ir
    ↓
SearchRequestIr
    ↓
Backend::search
    ↓
owned SearchResult
    ↓
SearchResult<T>
```

For writes:

```text
Rust value
    ↓
FlatBuffers mutation request
    ↓
*_to_ir
    ↓
operation IR
    ↓
Backend
```

C4 must not construct:

```text
FieldIr
DocumentIr
FilterIr
SearchRequestIr
...
```

directly.

That would bypass the semantic boundary.

---

# 22. Generic results

Expose:

```rust
pub struct SearchResult<T> {
    pub hits: Vec<Hit<T>>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
}
```

and:

```rust
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    pub document: T,
    pub highlights: Vec<HighlightResult>,
}
```

The exact field name may be `document` or `value`; choose one and freeze it.

Use `document`.

`execute::<T>()` requires:

```rust
T: DeserializeOwned
```

A projected result may deserialize into a projection-specific Rust struct rather than the full document type.

Deserialization failure is a client error, not a backend/search error.

---

# 23. Total hits

Expose the distinction already present in the portable contract:

```rust
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

pub enum TotalHitsRelation {
    Exact,
    LowerBound,
}
```

Do not collapse lower-bound totals into exact totals.

---

# 24. Facet/highlight results

Map the core result model mechanically.

Do not:

```text
sort facet values
merge facet buckets
rewrite highlights
insert markup
recalculate counts
```

in C4.

Preserve backend/core ordering and values.

---

# 25. Errors

Expose a single idiomatic Rust error surface:

```rust
searchkernel::Error
```

It must distinguish at least:

```text
portable Search Kernel error
document serialization error
document/result deserialization error
test/infrastructure construction error where applicable
```

Portable errors must preserve:

```text
ErrorCode
ErrorDetail.code
ErrorDetail.path
retryable
```

Do not convert semantic errors into opaque strings.

Provide:

```rust
std::error::Error
Display
Debug
```

implementations.

---

# 26. No duplicated semantics

C4 must not independently implement:

```text
field defaults
capability validation
analyzer validation
document type validation
filter validation
range validation
sortability checks
facet validation
ranking defaults
projection defaults
highlight validation
language normalization
query normalization
```

The facade constructs portable intent.

`searchkernel-ir` decides its meaning.

This rule is central.

---

# 27. No fake execution

C4 must not implement:

```text
token matching
BM25
filter execution
sorting
faceting
highlight generation
ranking
document persistence
```

Standalone tests use:

```text
RecordingBackend
ScriptedBackend
```

from C3.

---

# 28. Verification with `RecordingBackend`

For each public request path, verify:

```text
Rust API call
    ↓
portable request
    ↓
*_to_ir
    ↓
exact expected IR
```

At minimum cover:

```text
insert
upsert
patch
delete
get

basic search
search fields/weights
multiple filters -> AND
sort
facets
ranking
projection
highlighting
language
limit/offset
combined search request
```

Use structural IR equality.

Do not compare debug output.

---

# 29. Verification with `ScriptedBackend`

Verify the opposite direction:

```text
scripted backend result/error
    ↓
Rust product API
```

At minimum:

```text
insert success/error
upsert success/error
patch success/error
delete true/false
get document
get missing document

search hits
optional scores
total hits exact/lower-bound
facets
highlights
search backend error

document deserialization failure
portable error preservation
```

No test backend may derive a result from request contents.

---

# 30. Compile fixtures

Freeze compile-only fixtures for representative Rust syntax.

At minimum:

```rust
Field::text("title");

Field::keyword("brand")
    .facetable(true)
    .sortable(true);

SearchField::new("title").weight(2.0);

Patch::new()
    .set("price", 450.0)
    .unset("description");

field("price").lte(500.0);

Sort::desc("price");

Facet::new("brand").limit(10);

Ranking::Bm25;
```

and a complete fluent search:

```rust
let results = index
    .search("wireless headphones")
    .filter(field("in_stock").eq(true))
    .filter(field("price").lte(500.0))
    .facets(["brand", "category"])
    .limit(20)
    .execute::<Product>()?;
```

For standalone C4 fixtures, construct `index` using the testkit/backend injection path rather than real `Index::open()`.

---

# 31. Final integrated feel

C4 should be designed so I2 can later provide:

```rust
let index = Index::open("./products")?;

index.upsert("sku-42", &product)?;

let results = index
    .search("wireless headphones")
    .filter(field("in_stock").eq(true))
    .filter(field("price").lte(500.0))
    .facets(["brand", "category"])
    .limit(20)
    .execute::<Product>()?;
```

without redesigning:

```text
Index
mutation methods
search builder
Filter
Sort
Facet
Ranking
SearchResult<T>
Hit<T>
```

I2 should add lifecycle/execution plumbing, not redesign the product API.

---

# 32. Explicit non-goals

C4 does not add:

```text
Vespa dependency
filesystem/native lifecycle
bulk execution
transactions
presets
synonyms
curations
prefix/typo execution
grouping
vector search
async API
streaming API
arbitrary Vespa query/ranking escape hatches
```

Those require later explicit milestones.

Do not add them opportunistically.

---

# 33. Required structure

Keep `searchkernel` small.

Suggested shape:

```text
core/crates/searchkernel/src/
    lib.rs
    index.rs
    schema.rs
    document.rs
    patch.rs
    filter.rs
    search.rs
    result.rs
    error.rs
```

Split further only if materially useful.

Do not mirror every FlatBuffers table with another public Rust type.

The facade should contain ergonomic product concepts, not wire DTOs.

---

# 34. Acceptance

Run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

and the C4 verifier.

The verifier must prove:

```text
public compile fixtures pass

Rust facade produces exact expected IR through C3

scripted core/backend results map correctly to Rust values

portable errors retain canonical information

no Vespa dependency exists in searchkernel

no generated FlatBuffers type appears in the public Rust API

no *Ir type appears in the ordinary product-facing API

no search implementation exists in searchkernel
```

---

# 35. Definition of done

C4 is complete when:

```text
the ergonomic Rust schema API is frozen

CRUD/get/patch APIs are ergonomic and tested

the fluent search builder is frozen

presence-sensitive builder behavior is correct

Rust objects mechanically map into portable values

all requests pass through real *_to_ir conversion

no IR is fabricated by the facade

generic SearchResult<T>/Hit<T> mapping works

C3 RecordingBackend proves request parity

C3 ScriptedBackend proves result/error parity

searchkernel has no Vespa dependency

real Index::open remains an integration concern

all acceptance commands pass
```

The governing rule is:

> **C4 decides how Search Kernel feels to Rust programmers. `searchkernel-ir` remains the sole authority on what their requests mean.**
