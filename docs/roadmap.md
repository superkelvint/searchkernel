# Roadmap

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The project does not reimplement Vespa's search engine. It wraps the existing C++ engine and replaces only the parts of Vespa's Java request/configuration plane that are required to use that engine as an embedded library.

The roadmap is organized around three deliberately separate tracks:

```text
M-series — embedded Vespa capability
C-series — portable API, generated clients, and transports
I-series — integration between the two
```

The M-series and C-series are orthogonal. They meet only through the I-series.

The final semantic runtime architecture is:

```text
Portable API (FlatBuffers contract)
   │
   ▼
searchkernel-ir (*_to_ir)
   │
   ▼
owned Search Kernel IR
   │
   ▼
backend interface / lower_*
   │
   ▼
Vespa model and runtime
   │
   ├── persistence
   ├── indexing
   ├── attributes
   ├── matching
   └── ranking
```

The client/runtime architecture is:

```text
                     FlatBuffers contract
                            │
                       C6 generation
                            │
              ┌─────────────┴─────────────┐
              │                           │
     generated client core         generated JSON codec
              │                           │
         Transport API                    │
          /       \                       │
         /         \                      │
  NativeTransport  HttpTransport          │
       │                │                 │
       │                └──── HTTP/JSON ──┘
       │                         │
       │                    searchkerneld
       │
      C5
       │
       └───────────────┬─────────────────
                       │
              canonical FB dispatcher
                       │
              searchkernel-ir
```

The same daemon also supports direct binary HTTP:

```text
HTTP/FlatBuffers
      │
      └───────────────► canonical FB dispatcher
```

The important invariant is:

> Native clients and HTTP clients do not implement separate Search Kernel semantics.

For JSON specifically:

```text
JSON
  ↓
generated JSON codec
  ↓
canonical FlatBuffers
  ↓
one shared dispatcher
  ↓
searchkernel-ir
```

There must not be independent:

```text
JSON → IR
FlatBuffers → IR
```

paths.

The Portable API describes what the caller expressed. Search Kernel IR describes the fully validated and explicit meaning of that request. Backend lowering describes how that meaning is represented by the execution engine.

---

# Scope boundary

Search Kernel does need replacements for a small set of functionality that normally lives above Proton:

- index-time linguistics;
- query-time linguistics;
- construction of Vespa query trees;
- generation of the Proton configuration required by the supported schema subset;
- search-request and ranking properties.

Search Kernel does **not** initially reproduce Vespa's Java/container application layer, including:

- Vespa search chains;
- Vespa's Java HTTP/search APIs;
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

Search Kernel **does** define its own small transport surface where useful:

```text
embedded native transport
HTTP/JSON
HTTP/FlatBuffers
```

These expose the Search Kernel Portable API. They are not reimplementations of Vespa's Java request layer.

---

# M0 — Native SDK

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

# M1 — Rust ownership boundary

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
searchkernel-vespa-engine
    ↓
searchkernel-vespa-bindings
    ↓
native ABI
```

Verify ownership, destruction, errors, and native safety.

No Vespa C++ type or raw pointer appears in the public Rust API.

---

# M2 — Document mutation and attribute search

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

M0–M2 establish the native foundation. From this point onward the project deliberately splits into two orthogonal milestone series:

```text
M-series — embedded Vespa capability

    M3, M4, M5, ...

C-series — portable API, generated clients, and transports

    C1, C2, C3, ...
```

Neither numbering sequence implies an ordering relationship with the other.

For example:

```text
C6 does not depend on M6.
M8 does not depend on C8.
```

Each series has its own dependency chain and verifier suite.

They meet only through the integration series:

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

---

# M-series — Embedded Vespa capability

The M-series continues the work already started by M0–M2.

Its product is:

```text
searchkernel-vespa-engine
```

This is a safe, relatively faithful Rust interface to the embedded Vespa C++ search engine. It uses Vespa terminology where appropriate and does not try to provide the high-level product API.

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

This milestone defines the configuration model required by `searchkernel-vespa-engine`; it does **not** define the high-level Search Kernel product schema.

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
searchkernel-vespa-engine schema/config
        ↓
embedded configuration compiler
        ↓
Proton configuration
        ↓
DocumentDB
```

Use Vespa's Java config-model output as the oracle for selected schemas and validate resulting engine behavior.

---

## M7 — Vespa query compilation

Replace the minimum Java query-preparation path.

Expose a typed low-level Vespa query representation.

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
searchkernel-vespa-engine query
    ↓
field-aware query analysis
    ↓
Vespa query representation
    ↓
Matcher
```

M3 linguistics is reused for text terms.

YQL must be supported as well.

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

Expose Vespa capabilities rather than designing a Rust aggregation engine.

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

Harden `searchkernel-vespa-engine` independently of the high-level product.

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

# C-series — Portable API, generated clients, and transports

The C-series is orthogonal to the M-series.

Its product is the portable Search Kernel application API and everything needed to expose that API consistently across languages and transports:

```text
searchkernel-protocol / generated FlatBuffers artifacts
searchkernel-ir
searchkernel
searchkernel-ffi
searchkernel-json
searchkerneld
searchkernel-bindgen
language packages
```

The C-series must remain runnable and verifiable without linking `searchkernel-vespa-engine`.

It may use recording and scripted test backends, but must never grow a fake production search engine.

The C-series has one canonical semantic path:

```text
caller representation
        ↓
canonical FlatBuffers request
        ↓
shared dispatcher
        ↓
searchkernel-ir (*_to_ir)
        ↓
Search Kernel IR
```

---

## C1 — Portable API contract

Define the canonical language-neutral API model in FlatBuffers.

Primary artifacts:

```text
api/*.fbs
```

The FlatBuffers schema is normative.

It includes the complete portable request/response model, including concepts such as:

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

Every public client operation must be representable losslessly in this model.

### BFBS

The canonical `.fbs` schemas are compiled with the pinned `flatc` into BFBS schema artifacts:

```text
.fbs
  ↓ flatc --binary --schema
.bfbs
```

BFBS is the machine-readable compiler IR for downstream Search Kernel tooling.

The `.fbs` source remains normative.

BFBS must be generated reproducibly from the pinned FlatBuffers toolchain and must not be manually edited.

C6 consumes BFBS rather than implementing another `.fbs` parser.

---

## C2 — Search Kernel IR

Implement:

```text
searchkernel-ir
```

This crate performs Portable API → IR conversion and owns the Search Kernel IR:

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
explicit IR requests
```

For example:

```rust
Field::text("title")
```

must resolve to an explicit backend-independent value such as:

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

The architectural rule is:

> The Portable API describes what the caller expressed. Search Kernel IR describes the fully validated and explicit meaning of that request. Backend lowering describes how that meaning is represented by the execution engine.

---

## C3 — Standalone conformance testkit

Implement:

```text
searchkernel-testkit
```

with two deliberately non-searching test backends:

```text
RecordingBackend
    records canonical Search Kernel IR operations

ScriptedBackend
    returns predetermined canonical responses
```

This verifies:

```text
Portable API
    ↓
Search Kernel IR
```

and:

```text
portable response
    ↓
public result/error
```

Do not implement BM25, fuzzy matching, faceting, prefix matching, ranking, or another search engine in the testkit.

---

## C4 — Rust high-level API

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

The FFI transports the canonical FlatBuffers request/response envelopes.

Conceptually:

```text
NativeTransport
    ↓
sk_call(handle, request_bytes)
    ↓
canonical FlatBuffers dispatcher
```

The native ABI is transport plumbing, not the semantic definition of Search Kernel.

Adding a portable operation should normally not require adding another per-operation C function.

---

## C6 — BFBS-driven generation and portable client core

C6 is the central client-generation milestone.

Implement:

```text
tools/searchkernel-bindgen
```

The generator consumes the canonical BFBS schema IR:

```text
api/*.fbs
   ↓
pinned flatc
   ↓
*.bfbs
   ↓
searchkernel-bindgen
```

`searchkernel-bindgen` must not parse `.fbs` source itself.

### Generated outputs

Generate the mechanical portions of the portable client/runtime surface:

```text
request/response models
enums
symbolic union adapters
serialization glue
portable error mappings
operation scaffolding
generated client core
Transport abstraction scaffolding
language conformance fixtures
documentation stubs
canonical JSON codec artifacts
JSON Schema / OpenAPI-compatible metadata
```

Language-specific façades remain small and idiomatic.

### Generated client core

The generated client layer should conceptually target:

```text
Client
  │
  ├── insert(...)
  ├── upsert(...)
  ├── patch(...)
  ├── delete(...)
  ├── get(...)
  ├── search(...)
  ├── analyze(...)
  ├── describe(...)
  └── inspect_query(...)
         │
         ▼
         Transport boundary
         ├── synchronous `Transport`
         └── asynchronous `AsyncTransport`
              │
         ▼
 call(canonical request) → canonical response
```

The embedded engine remains synchronous. `AsyncTransport` and its corresponding
async client surface are additive portable-client APIs; they do not wrap the
blocking native engine in futures or require a runtime dependency.

The exact host-language shape may differ idiomatically, but operation coverage and portable meaning must be generated from the same BFBS source.

### Transport abstraction

Clients are built around a transport boundary, conceptually:

```text
Transport
    call(request) -> response
```

Transport implementations may differ.

Semantics may not.

### Canonical JSON codec

C6 also generates the JSON ↔ FlatBuffers codec used by HTTP.

The required path is:

```text
JSON
  ↓
generated JSON codec
  ↓
canonical FlatBuffers
  ↓
shared dispatcher
```

Never:

```text
JSON → IR
```

directly.

Responses follow the inverse path:

```text
canonical FlatBuffers response
  ↓
generated JSON codec
  ↓
JSON
```

### JSON union representation

Generated nested FlatBuffers unions are represented as symbolic one-key objects.

Examples:

```json
{"string":"x"}
```

```json
{"term_query":{"field":"title","term":"rust"}}
```

A union object must contain exactly one recognized variant key.

Reject:

```text
zero variant keys
multiple variant keys
unknown variant keys
```

Do not expose FlatBuffers discriminator fields such as:

```text
payload_type
kind_type
expr_type
```

in the public JSON representation.

### JSON scalar policy

Freeze these rules:

```text
bool            → JSON boolean
bounded 32-bit integers/counts
                → JSON number
finite float32/float64
                → JSON number

portable int64  → canonical base-10 JSON string
portable uint64 → canonical base-10 JSON string

bytes           → base64 JSON string
```

NaN and ±Infinity are rejected as non-portable values where the portable/core semantics reject them.

`int64` / `uint64` strings must round-trip the full domain without relying on JavaScript or PHP numeric precision.

### Presence

The JSON codec must preserve all distinctions represented by the FlatBuffers contract:

```text
absent
explicit false
explicit zero
explicit empty string
explicit empty list
present non-empty value
```

JSON `null` is not a generic synonym for absence.

A field may accept JSON null only where the Portable API itself represents a semantic null value.

### Strictness

Generated JSON decoding should reject:

```text
unknown fields
duplicate fields
invalid enum names
invalid union forms
out-of-range integers
malformed canonical integer strings
```

### Generated, not handwritten

Per-message JSON mappings must not be handwritten.

The only handwritten JSON policy should be small global rules that BFBS cannot decide by itself, such as:

```text
union presentation
int64/uint64 textual representation
bytes encoding
null policy
unknown-field policy
operation naming conventions
```

### Coverage invariant

For every Portable API operation:

```text
Portable operation exists
        ⇕
canonical request union variant exists
        ⇕
canonical response union variant exists
        ⇕
shared dispatcher covers it
        ⇕
generated client core covers it
        ⇕
generated JSON codec covers it
```

Adding a new schema construct must either generate complete support or fail generation explicitly.

Silently incomplete generated support is forbidden.

---

## C7 — Native embedded client transport

Implement the reusable native transport used by languages that embed Search Kernel in-process.

Architecture:

```text
language facade
      ↓
generated client core
      ↓
NativeTransport
      ↓
C5 native ABI
      ↓
canonical FlatBuffers dispatcher
```

Representative targets should prove the pattern.

Python and Go are good initial proofs because they exercise substantially different native-integration ecosystems.

Node.js and Java may follow the same architecture.

### Requirements

Native transports must remain thin.

They own:

```text
native library loading
handle lifecycle
FFI call plumbing
buffer ownership
language/runtime-specific packaging
```

They do not own:

```text
Search Kernel defaults
filter semantics
ranking semantics
portable validation
operation definitions
portable error definitions
```

Those remain in the shared generated/core layers.

---

## C8 — `searchkerneld` and HTTP transports

Implement:

```text
searchkerneld
```

as a small transport host for the same Portable API.

It is not a second Search Kernel semantic implementation.

### HTTP/JSON

```text
HTTP/JSON
    ↓
generated JSON codec
    ↓
canonical FlatBuffers
    ↓
shared dispatcher
```

### HTTP/FlatBuffers

Also support direct binary requests:

```text
HTTP/FlatBuffers
    ↓
canonical FlatBuffers
    ↓
shared dispatcher
```

This exists for workloads where JSON is inherently inefficient, including:

```text
bulk indexing
large documents
dense vectors / embeddings
tensor-heavy payloads
large responses
```

JSON remains the simple interoperable transport.

FlatBuffers-over-HTTP remains the high-throughput transport.

### Server responsibilities

`searchkerneld` may own transport concerns such as:

```text
HTTP routing
content negotiation
request-size limits
timeouts
connection handling
index/session addressing
transport-level errors
```

It must not independently implement request semantics.

There must not be handwritten per-operation semantic handlers that bypass the canonical FlatBuffers dispatcher.

### C-series independence

C8 must be verifiable using C3 recording/scripted backends.

It does not require Vespa.

---

## C9 — First-class language façades and packages

Deliver first-class language packages as thin façades over the generated client/runtime architecture.

Initial targets:

```text
Rust
Python
Node.js / TypeScript
PHP
Go
Java
```

The important distinction is transport preference, not separate semantics.

### Preferred initial transports

```text
Rust                 embedded/direct
Python               NativeTransport
Node.js / TypeScript NativeTransport
Go                   NativeTransport
Java                 NativeTransport

PHP                  HttpTransport
```

Where useful, languages may expose additional transports later:

```text
NativeTransport
HttpJsonTransport
HttpFlatBuffersTransport
```

but the user-facing semantics remain the same.

### PHP

PHP should not require a native extension for the initial first-class client.

Primary path:

```text
PHP facade
    ↓
generated client support
    ↓
HTTP/JSON
    ↓
searchkerneld
```

A higher-throughput PHP path may use HTTP/FlatBuffers without requiring a Search Kernel PHP native extension.

### Language façades

Language syntax may be idiomatic.

Examples include:

```text
Rust builders
Python operators / kwargs
TypeScript options
PHP fluent methods
Go functional options
Java builders / records
```

These are presentation differences only.

Defaults, validation, request meaning, portable errors, and operation coverage must not be independently redefined by each SDK.

---

## C10 — Cross-language and cross-transport portable conformance

Complete the standalone C-series by proving that all supported clients and transports express the same Portable API semantics before Vespa is involved.

Run the same scenario corpus through representative paths such as:

```text
Rust high-level API

Python NativeTransport
Go NativeTransport
Node NativeTransport
Java NativeTransport

PHP HTTP/JSON
raw HTTP/JSON
HTTP/FlatBuffers
```

against:

```text
RecordingBackend
ScriptedBackend
```

### Request-side invariant

All equivalent client operations must produce the same canonical IR:

```text
different languages
different syntax
different transports
        ↓
canonical FlatBuffers
        ↓
same *_to_ir()
        ↓
identical Search Kernel IR
```

### Response-side invariant

The same scripted canonical response/error must produce equivalent language-level meaning across all supported clients and transports.

C10 is the point at which:

> Adding another language is packaging and idiomatic façade work, not another Search Kernel semantic implementation.

---

# I-series — Integration

The I-series is the only place the C-series and M-series meet.

Its core implementation is:

```text
searchkernel-ir-adapter
```

which may depend on both:

```text
searchkernel-ir
searchkernel-vespa-engine
```

No other crate should need to understand both public object models.

Integration may begin incrementally as matching C- and M-series capabilities become available.

It does not need to wait for C10 or M11.

---

## I1 — Adapter skeleton and dependency firewall

Create the integration crate and freeze the backend boundary.

Prove:

```text
C-series crates do not import searchkernel-vespa-engine
M-series crates do not import searchkernel-ir
integration crate imports both
```

No meaningful search capability is required yet.

---

## I2 — Index lifecycle and schema

Compile:

```text
ResolvedIndexDefinition
    ↓
searchkernel-vespa-engine configuration
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

High-level facet semantics remain owned by `searchkernel-ir`.

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

## I9 — Cross-language and cross-transport real-engine conformance

Take the C10 client/transport matrix and run it against the real integrated Vespa backend.

Representative paths include:

```text
Rust embedded
Python embedded
Node embedded
Go embedded
Java embedded

PHP HTTP/JSON
raw HTTP/JSON
HTTP/FlatBuffers
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

Where vector/tensor support exists, include representative binary-heavy payloads and compare JSON and FlatBuffers transport semantics.

All clients and transports must produce equivalent canonical outcomes.

---

## I10 — Packaging and product hardening

Produce clean-machine packages for supported platforms and languages.

Test without:

```text
Vespa source
Vespa development container
Vespa build tools
Vespa Java runtime/services
```

Record:

```text
Search Kernel version
Portable API version
BFBS/schema version
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
HTTP overhead
JSON codec overhead
binary HTTP throughput
```

---

# Parallel execution

The intended development shape after M2 is:

```text
M TRACK

M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11


C TRACK

C1 -> C2 -> C3 -> C4 -> C5 -> C6 -> C7 -> C8 -> C9 -> C10
```

These arrows mean dependencies inside each track.

There are no implicit diagonal dependencies.

For example:

```text
C6 does not require M6.
C8 does not require M8.
M7 does not require C7.
```

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

C6–C10 may continue independently while I2–I8 progress.

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

For every series, checked-in verifier contracts, scenarios, and expected outputs define the frozen specification.

---

## C-series

Authority is:

```text
canonical FlatBuffers schema
+
BFBS generated from that schema
+
canonical Search Kernel IR
+
portable conformance corpus
```

Verification asks:

```text
did this public language operation produce the correct canonical request?

did JSON decode into the same canonical FlatBuffers request?

did all transports reach the same *_to_ir() implementation?

did equivalent operations produce identical canonical IR?

did canonical responses produce equivalent language-level results/errors?
```

No real search engine is required for C-series acceptance.

### Generation verification

C6 must include reproducibility checks conceptually equivalent to:

```text
regenerate BFBS
regenerate bindgen artifacts
git diff --exit-code
```

A schema change without regenerated artifacts must fail CI.

### BFBS coverage

The generator must fail on FlatBuffers features it does not understand.

It must not silently skip unknown fields, unions, attributes, or schema features.

### JSON equivalence

For every portable conformance fixture:

```text
FlatBuffers A
    ↓
FB → JSON
    ↓
JSON → FB
    ↓
FlatBuffers B
```

verify semantic equivalence.

Where applicable, assert:

```text
A → *_to_ir()
==
B → *_to_ir()
```

Byte-for-byte FlatBuffer identity is not required unless canonical byte encoding is explicitly guaranteed.

---

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

# Client parity rule

Once a language package or transport becomes first-class, a Portable API change is not complete until:

```text
canonical .fbs updated
+
BFBS regenerated
+
searchkernel-bindgen outputs regenerated
+
searchkernel-ir semantics updated
+
affected generated client/runtime artifacts refreshed
+
affected transport conformance passes
+
affected language façade conformance passes
```

Language syntax may differ idiomatically.

Semantic meaning may not drift.

There must never be independent definitions such as:

```text
Python defaults
Go defaults
PHP defaults
HTTP defaults
native defaults
```

for the same portable operation.

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

## After C5

The portable in-process binary ABI is proven.

Foreign runtimes can use one stable request/response byte transport rather than a per-operation C ABI.

## After C6

The schema-driven client architecture is proven:

```text
.fbs
 ↓
BFBS
 ↓
searchkernel-bindgen
 ↓
generated client/runtime + JSON codec
```

Adding an operation no longer requires independently updating each client protocol implementation.

## After C8

Both transport families are proven without Vespa:

```text
embedded/native
HTTP/JSON
HTTP/FlatBuffers
```

and all converge on the same canonical FlatBuffers dispatcher.

## After C10

The portable multi-language product surface is proven independently of Vespa.

Different languages and transports produce the same Search Kernel IR and consume the same portable results/errors.

## After M8

The low-level wrapper can construct schemas/queries and execute useful local searches without the Vespa Java runtime.

## After I4/I5

The high-level API performs real lexical search through Vespa.

This is the first integrated product checkpoint.

## After I9

All first-class languages and transports pass the same real-engine conformance scenarios.

This is the multi-language, multi-transport product checkpoint.

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
- additional first-class language façades such as C#, Ruby, and Swift;
- optional additional transports such as gRPC if real usage justifies them;
- dependency/runtime slimming.

New language packages should reuse the C6–C10 architecture rather than introduce new semantic implementations.

New transports must converge on the same canonical FlatBuffers request/response path before semantic processing.

---

# Guiding rules

When choosing between:

```text
represent Vespa faithfully
```

and:

```text
invent a more general Search Kernel backend abstraction
```

prefer the first until real usage demonstrates why the second is necessary.

When choosing between:

```text
derive client/transport behavior from the canonical FlatBuffers schema
```

and:

```text
maintain another language- or transport-specific protocol definition
```

always choose the first.

The desired end state is:

```text
one portable contract
one BFBS compiler IR
one API → IR semantic implementation
one integrated Vespa backend
many generated façades and transports
```

not many independently maintained Search Kernel implementations.
