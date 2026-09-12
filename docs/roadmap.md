# Roadmap

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The project does not reimplement Vespa's search engine. It wraps the existing C++ engine and replaces only the parts of Vespa's Java request/configuration plane that are required to use that engine as an embedded library.

The runtime target is:

```text
Rust API
   │
   ├── document preparation
   ├── query preparation
   └── configuration generation
   │
   ▼
native boundary
   │
   ▼
Vespa C++
   │
   ├── persistence
   ├── indexing
   ├── attributes
   ├── matching
   └── ranking
```

The roadmap is ordered primarily by integration risk.

## Scope boundary

Search Kernel does need replacements for a small set of functionality that normally lives above Proton:

- index-time linguistics;
- query-time linguistics;
- construction of Vespa query trees;
- generation of the Proton configuration required by the supported schema subset;
- search-request and ranking properties.

Search Kernel does **not** initially reproduce:

- YQL;
- Vespa search chains;
- HTTP APIs;
- query profiles;
- spelling correction;
- synonym systems;
- distributed dispatch;
- result merging across nodes;
- MessageBus;
- Slobrok;
- Config Server;
- Config Proxy;
- Java file distribution.

Those may be considered independently later.

---

## M0 — Native SDK

Package the working embedded Vespa prototype as a reproducible native dependency.

Produce a native SDK containing:

```text
include/
lib/
share/
manifest.json
```

It must be usable outside the Vespa development container.

Verify:

- engine startup;
- engine shutdown;
- native ABI version;
- runtime dependencies;
- repeated open/close;
- no Vespa source checkout required by the consumer.

---

## M1 — Rust ownership boundary

Safely own the native engine from Rust.

Target:

```rust
let engine = Engine::open(path, config)?;
drop(engine);
```

Establish:

```text
searchkernel
    ↓
searchkernel-vespa
    ↓
searchkernel-vespa-sys
    ↓
native ABI
```

Verify ownership, destruction, errors, and native safety.

No Vespa C++ type or raw pointer appears in the public Rust API.

---

## M2 — Document mutation and attribute search

Expose the first real Vespa data path through Rust.

Required sequence:

```text
put
search
update
search
delete
search
```

Use the existing integer-attribute fixture and the real Vespa persistence/matching paths.

This milestone proves that non-trivial data can cross the Rust/native boundary.

M0–M2 are already defined by the existing verifier harness and should not be renumbered.

---

# Development split after M2

M0–M2 establish the native foundation. From this point onward the project deliberately splits into **two orthogonal milestone series**:

```text
M-series — embedded Vespa capability

    M3, M4, M5, ...

C-series — high-level client/API platform

    C1, C2, C3, ...
```

Neither numbering sequence implies an ordering relationship with the other.

For example:

```text
C4 does not depend on M4.
M7 does not depend on C7.
```

Each series has its own dependency chain and verifier suite.

They meet only through a third, narrow integration series:

```text
             C-series
                │
                ▼
             I-series
                ▲
                │
             M-series
```

The architectural rule is:

```text
client/high-level work can compile and verify without Vespa

Vespa-wrapper work can compile and verify without the high-level client

only the integration layer depends on both
```

This allows both sides to be developed in parallel without building a second search engine.

---

# M-series — Embedded Vespa capability

The M-series continues the work already started by M0–M2.

Its product is:

```text
searchkernel-vespa
```

This is a safe, relatively faithful Rust interface to the embedded Vespa C++ search engine. It uses Vespa terminology where appropriate and does not try to provide the Algolia/Typesense-style product API.

## M3 — Linguistics compatibility

Replace the minimum Java linguistics functionality required by the embedded engine.

Cover both sides:

```text
DOCUMENT

raw string
   ↓
tokenize
normalize
stem
annotate positions/terms
   ↓
Vespa C++ indexing


QUERY

raw query term/text
   ↓
tokenize
normalize
stem
   ↓
Vespa query terms
```

The initial scope should remain small.

Automatic language detection, synonyms, spelling correction, and a general analyzer framework are not required.

Use the real Vespa Java linguistics implementation as the golden oracle for the supported subset.

The key invariant is:

> Rust index-time and query-time analysis must produce semantics compatible with normal Vespa analysis.

---

## M4 — Text indexing and matching

Use M3 linguistics to drive the real C++ text path.

Support:

- text fields;
- term queries;
- phrase queries;
- positions;
- BM25;
- updates;
- deletes.

The existing precompiled native configuration fixture may still be used.

This milestone must not retain the temporary toy ASCII tokenizer used by the original spike.

Verify:

```text
Rust analysis
    ↓
annotated Vespa document/query
    ↓
real Proton index/matcher
    ↓
expected results
```

At this point the complete Rust → Vespa text path is proven.

---

## M5 — Persistence and reopen

Verify the wrapped engine as a persistent embedded index.

Exercise:

```text
open
put/update/delete
search
close

open same directory
search
update
close
```

Also establish persistent fixtures for future Vespa-version upgrades.

Test graceful reopen first, then crash/recovery behavior where practical.

Search Kernel does not invent different durability semantics; it documents and verifies what the embedded Vespa engine actually provides.

---

## M6 — Vespa schema/configuration

Remove the dependency on pre-generated Java configuration for the supported low-level schema subset.

This milestone defines the configuration model required by `searchkernel-vespa`; it does **not** define the high-level Search Kernel product schema.

Initial capabilities:

```text
text
string / keyword
integer
floating point
boolean where practical

index
attribute
stored/summary
```

Compile this representation into the configuration required to construct and run the embedded C++ engine.

Conceptually:

```text
searchkernel-vespa schema/config
        ↓
embedded configuration compiler
        ↓
Proton configuration
        ↓
DocumentDB
```

Use Vespa's Java config-model output as the oracle for selected fixtures, comparing semantic configuration and resulting behavior rather than implementation details.

---

## M7 — Vespa query compilation

Replace the minimum Java query-preparation path.

Expose a typed **low-level Vespa query representation**.

Initial operators:

```text
term
phrase

AND
OR
NOT

equality
range
exists
```

Compilation is:

```text
searchkernel-vespa query
    ↓
field-aware query analysis
    ↓
Vespa query representation
    ↓
Matcher
```

M3 linguistics is reused for text terms.

There is no YQL compatibility requirement.

This API is distinct from the C-series high-level `index.search(...).filter(...)` API.

---

## M8 — Local search request and results

Expose the remainder of the ordinary local Vespa request/result path.

Initial targets:

```text
limit
offset where supported
sorting
stored/summary fields
result hits
scores
total-hit information available from the engine
```

Support structured filtering through M7.

Wrap the local summary/docsums path where required.

Do not reproduce distributed search dispatch or distributed result merging.

---

## M9 — Attributes, grouping, and facets

Broaden the attribute support initially proven in M2.

Cover useful mutable attribute types and operations:

```text
attribute updates
filtering
sorting
ranking access
facet/group counts
basic aggregations where practical
```

Expose Vespa capabilities rather than designing a second aggregation engine.

---

## M10 — Ranking integration

Expose Vespa's existing ranking system.

Initial targets:

```text
BM25 configuration
field-specific ranking
attribute features
query features
rank properties
native rank features
ranking expressions
feature inspection
```

Ranking execution remains entirely in Vespa C++.

For ranking assets such as constants or models, embedded operation uses explicit local resources rather than recreating Vespa's file-distribution service.

---

## M11 — Low-level wrapper hardening

Harden `searchkernel-vespa` independently of the high-level product.

Cover:

- lifecycle races;
- malformed inputs;
- native failures;
- repeated reopen;
- supported concurrency;
- memory safety;
- resource leaks;
- native sanitizers;
- long-running mutation/search workloads.

This milestone does not require C-series clients.

---

# C-series — High-level API and language clients

The C-series is orthogonal to the M-series.

Its product is the portable Search Kernel application API and its language clients:

```text
searchkernel-core
searchkernel
searchkernel-ffi
language bindings
```

The C-series must remain runnable and verifiable without linking `searchkernel-vespa`.

It may use recording and scripted test backends, but must never grow a fake production search engine.

## C1 — Portable API contract

Define the canonical language-neutral API model.

Primary artifact:

```text
api/searchkernel.proto
```

Initial model includes:

```text
IndexDefinition
FieldDefinition
Analyzer
SearchField

InsertRequest
UpsertRequest
PatchRequest
DeleteRequest
GetRequest

SearchRequest
FilterExpr
SortExpr
FacetRequest
RankingPolicy

SearchResult
Hit
FacetResult
HighlightResult

AnalyzeRequest
DescribeRequest
InspectQueryRequest

portable errors
```

The protobuf is internal plumbing, not the syntax application developers use directly.

Every public client operation must be representable losslessly in this model.

---

## C2 — High-level semantic core

Implement:

```text
searchkernel-core
```

This crate owns Search Kernel semantics:

```text
stable defaults
schema validation
request normalization
field capabilities
analyzer identities
filter validation
ranking policies
facet requests
prefix/typo policy
portable errors
resolved requests
```

For example:

```rust
Field::text("title")
```

must resolve to an explicit backend-independent semantic value such as:

```text
ResolvedField {
    kind: Text,
    analyzer: StandardV1,
    stored: true,
    searchable: true,
    ...
}
```

No Vespa crate may be a dependency.

---

## C3 — Standalone conformance testkit

Implement:

```text
searchkernel-testkit
```

with two deliberately non-searching test backends:

```text
RecordingBackend
    records canonical resolved operations

ScriptedBackend
    returns predetermined canonical responses
```

This verifies:

```text
public API
    ↓
resolved portable request
```

and:

```text
portable response
    ↓
public result/error
```

Do not implement BM25, fuzzy matching, faceting, prefix matching, ranking, or another search engine in the testkit.

---

## C4 — Rust high-level client

Implement the ergonomic Rust product API.

Target vocabulary:

```text
Index
IndexDefinition
Field
Analyzer
SearchField
DocumentId
Patch
Filter
Ranking
Sort
Facet
SearchResult<T>
Hit<T>
```

Target feel:

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

Verification uses C3 only.

---

## C5 — Portable native ABI

Implement:

```text
searchkernel-ffi
```

with a deliberately tiny language-neutral ABI based on:

```text
opaque handles
request bytes
response bytes
status/error codes
explicit buffer ownership
close/free
```

Do not mirror every high-level object as a C function.

The FFI transports the canonical C1 request/response model.

---

## C6 — Binding generator

Implement:

```text
tools/searchkernel-bindgen
```

Generate mechanical language-client code from the portable API contract:

```text
request/response models
enums/tagged unions
errors
serialization
API documentation stubs
conformance fixtures
```

Language-specific facades remain small and idiomatic.

---

## C7 — Python client

Provide an idiomatic Python API over the portable runtime.

Example shape:

```python
results = index.search(
    "wireless headphones",
    filters=[
        field("in_stock") == True,
        field("price") <= 500,
    ],
    facets=["brand", "category"],
    limit=20,
)
```

Its acceptance suite proves semantic equivalence with the canonical C1 contract, not real Vespa search yet.

---

## C8 — Node.js / TypeScript client

Provide an idiomatic Node/TypeScript API with generated types and native packaging plumbing.

Example shape:

```typescript
const results = await index.search("wireless headphones", {
  filter: all(
    field("in_stock").eq(true),
    field("price").lte(500),
  ),
  facets: ["brand", "category"],
  limit: 20,
});
```

Verify against the same portable conformance suite.

---

## C9 — PHP client

Provide an idiomatic PHP API.

Example shape:

```php
$results = $index
    ->search("wireless headphones")
    ->filter(field("in_stock")->eq(true))
    ->filter(field("price")->lte(500))
    ->facets(["brand", "category"])
    ->limit(20)
    ->get();
```

Use a native extension for first-class packaging; PHP FFI may be useful for development but is not the production semantic boundary.

---

## C10 — Go client

Provide an idiomatic Go API rather than transliterating the Rust builder syntax.

Example shape:

```go
results, err := index.Search(
    ctx,
    "wireless headphones",
    searchkernel.WithFilter(
        searchkernel.All(
            searchkernel.Field("in_stock").Eq(true),
            searchkernel.Field("price").LTE(500),
        ),
    ),
    searchkernel.WithFacets("brand", "category"),
    searchkernel.WithLimit(20),
)
```

The embedded implementation may require cgo, but the public semantics remain the same portable C1 contract.

---

# I-series — Integration

The I-series is the only place the two orthogonal tracks meet.

Its core implementation is:

```text
searchkernel-vespa-backend
```

which may depend on both:

```text
searchkernel-core
searchkernel-vespa
```

No other crate should need to understand both public object models.

Integration may begin incrementally as matching C- and M-series capabilities become available. It does not need to wait for C10 or M11.

## I1 — Adapter skeleton and dependency firewall

Create the integration crate and freeze the backend boundary.

Prove:

```text
C-series crates do not import searchkernel-vespa
M-series crates do not import searchkernel-core
integration crate imports both
```

No meaningful search capability is required yet.

---

## I2 — Index lifecycle and schema

Compile:

```text
ResolvedIndexDefinition
    ↓
searchkernel-vespa configuration
```

Map high-level index creation/opening onto the low-level engine lifecycle.

This is the first real implementation of `Index::create` / `Index::open` against Vespa.

---

## I3 — Document operations

Map:

```text
insert
upsert
patch
get
delete
bulk writes
```

onto the already-verified low-level mutation path.

Verify high-level mutation semantics end-to-end through real Vespa.

---

## I4 — Lexical search and filters

Map canonical high-level search requests onto M3/M4/M7 primitives.

Cover:

```text
search text
search fields and weights
term analysis
phrases
boolean filters
exact filters
range filters
```

The high-level API does not expose low-level Vespa query types.

---

## I5 — Results, sorting, projection, pagination, highlighting

Map the C-series result contract onto M8 capabilities.

Cover:

```text
hits
total hits
stored fields
projection
sorting
limit/offset
highlighting/snippets where supported
```

---

## I6 — Facets and grouping

Compile high-level facet/group requests onto M9 grouping capabilities.

Examples:

```rust
.facets(["brand", "category"])
.group_by("product_id")
```

High-level facet semantics remain owned by the C-series.

---

## I7 — Ranking

Compile high-level ranking policies onto M10 ranking capabilities.

Initial mappings include:

```text
DefaultRelevanceV1
BM25
search-field weights
business-field tie breakers
explicit hard sort
```

The high-level API does not expose arbitrary Vespa rank expressions.

---

## I8 — Higher-level search behavior

Integrate Search Kernel-owned semantics that compose several native capabilities:

```text
prefix search
typo tolerance
synonyms
presets
curations
facet-value search
```

Some compile into query rewrites.

Some may require generated companion representations.

Some may use native ranking features.

All remain high-level Search Kernel semantics.

---

## I9 — Cross-language real-engine conformance

Run the same product scenarios through every first-class client against real Vespa:

```text
Rust
Python
Node.js / TypeScript
PHP
Go
```

A representative fixture should cover:

```text
create index
bulk insert
search
filter
facet
sort
update
search
delete
close
reopen
search
```

All clients must produce equivalent canonical outcomes.

---

## I10 — Packaging and product hardening

Produce clean-machine packages for supported platforms and languages.

Test without:

```text
Vespa source
Vespa development container
Vespa build tools
Java runtime
```

Record:

```text
Search Kernel version
portable API version
native ABI version
pinned Vespa revision
target platform
```

Then harden the complete product for:

```text
lifecycle races
malformed requests
native failures
crash/recovery
concurrency
resource leaks
long-running workloads
package size
startup time
RSS/PSS
indexing throughput
query latency
```

---

# Parallel execution

The intended development shape after M2 is not:

```text
M3 -> M4 -> ... -> M10 -> C1 -> C2 -> ...
```

It is:

```text
M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11

C1 -> C2 -> C3 -> C4 -> C5 -> C6 -> C7 -> C8 -> C9 -> C10
```

running independently.

Integration trails whichever capabilities are ready:

```text
C1/C2 + M6
    -> I2 schema mapping can begin

C4 + M2/M5
    -> I3 writes can begin

C4 + M3/M4/M7
    -> I4 lexical search can begin

C4 + M8
    -> I5 result integration can begin

C4 + M9
    -> I6 facets can begin

C4 + M10
    -> I7 ranking can begin
```

There is deliberately no statement such as:

```text
C6 requires M6
```

unless an integration milestone explicitly says so.

---

# Verification strategy

## M-series

Authority remains the existing verifier/oracle model:

```text
M0–M2    direct Vespa C++ behavior
M3       Vespa Java linguistics
M4–M5    direct embedded C++ engine
M6       Vespa Java config-model + C++ behavior
M7       Vespa query preparation / canonical query behavior
M8–M10   direct C++ behavior, with Java oracle where Java currently
          defines request/config preparation
```

Java may remain a test/oracle dependency.

It must not become a runtime dependency.

## C-series

Authority is the frozen portable API contract and canonical high-level semantics.

Verification asks:

```text
did this public language operation produce the correct canonical request?

did this canonical response produce the correct language-level result/error?
```

No real search engine is required for C-series acceptance.

## I-series

Authority is the combination of:

```text
frozen C-series semantics
+
verified M-series capability
+
end-to-end product scenarios
```

The integration layer may translate semantics.

It may not redefine either side.

---

# Checkpoints

## After M4

The low-level track has proven:

```text
Rust
  ↓
Rust linguistics
  ↓
native boundary
  ↓
Vespa text indexing + matching
```

using a pre-generated configuration fixture.

## After C4

The high-level Rust product API is usable and testable without Vespa.

This is the first API/product checkpoint.

## After C6

The portable client-generation architecture is proven.

Adding a language no longer means independently redefining Search Kernel semantics.

## After M8

The low-level wrapper can construct schemas/queries and execute useful local searches without the Vespa Java runtime.

## After I4/I5

The high-level Rust API performs real lexical search through Vespa.

This is the first integrated product checkpoint.

## After I9

All first-class language clients pass the same real-engine conformance scenarios.

This is the multi-language product checkpoint.

---

# Vespa upgrades

Vespa remains pinned.

Updating Vespa is an explicit compatibility exercise, not routine dependency churn.

Every upgrade must verify:

- C++ adapter compatibility;
- native ABI behavior;
- Java-replacement parity where relevant;
- search behavior;
- persisted-index compatibility.

Remaining on the current revision is always a valid outcome.

---

# Later

Possible later work includes:

- richer linguistics;
- automatic language detection;
- vectors and HNSW;
- tensors;
- ONNX/model ranking;
- richer grouping;
- Java and C# first-class clients;
- Ruby and Swift clients;
- optional `searchkerneld` protocol transport;
- dependency/runtime slimming.

These extend the M-, C-, or I-series according to ownership. They do not collapse the tracks into one milestone sequence.
