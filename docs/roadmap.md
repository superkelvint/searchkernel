# Roadmap — intent and sequencing

> This document describes intended work and the sequencing decisions behind
> it. It is not the live project status. For the current repository state,
> read [docs/status.md](status.md). For architectural invariants, read
> [docs/architecture.md](architecture.md). For the reason older milestone and
> transport references remain, read [docs/history.md](history.md).

The roadmap was originally written when M11 was the planned end of the
low-level hardening track and the gRPC/Python migration was future work. That
sequence is now historical at those points: M12, M13, `searchkerneld`, and the
Python gRPC/facade path exist in the repository. The sections below remain
useful as design intent and future sequencing, but their presence is not proof
that work is pending or complete.

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The project does not reimplement Vespa's search engine. It wraps the existing C++ engine and replaces only the parts of Vespa's Java request/configuration plane required to use that engine as a local product.

The roadmap is organized around three deliberately separate tracks:

```text
M-series — embedded Vespa capability
C-series — portable API, Rust product API, RPC clients
I-series — integration between the portable semantics and Vespa
```

The M-series and C-series remain orthogonal. They meet only through the I-series.

The final semantic runtime architecture remains:

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

The client/runtime architecture is now deliberately asymmetric:

```text
RUST

Rust application
    │
    ▼
idiomatic Rust Search Kernel API
    │
    ▼
Search Kernel IR
    │
    ▼
Vespa adapter/runtime


NON-RUST

language facade
    │
    ▼
flatc-generated FlatBuffers models
    │
    ▼
canonical RequestEnvelope bytes
    │
    ▼
generated gRPC client
    │
    ▼
UDS by default / TCP where appropriate
    │
    ▼
searchkerneld
    │
    ▼
canonical FlatBuffer dispatcher
    │
    ▼
searchkernel-ir
```

FlatBuffers remains the sole portable domain contract.

The gRPC schema is transport-only and intentionally tiny, conceptually:

```proto
message Frame {
  bytes payload = 1;
}

service SearchKernel {
  rpc Execute(Frame) returns (Frame);
}
```

The protobuf must not duplicate Search Kernel requests, query expressions, ranking models, values, errors, or operation unions.

The important invariant is:

> Rust, gRPC clients, and any optional browser/JSON edge all converge on the same portable semantics in `searchkernel-ir`.

There must not be independent transport-specific semantic implementations.

---

# Scope boundary

Search Kernel does need replacements for a small set of functionality that normally lives above Proton:

- index-time linguistics;
- query-time linguistics;
- construction of Vespa query trees;
- generation of Proton configuration required by the supported schema subset;
- search-request and ranking property construction.

Search Kernel does **not** initially reproduce Vespa's Java/container application layer, including:

- Vespa search chains;
- Vespa's Java HTTP/search APIs;
- query profiles as a product abstraction;
- spelling correction unless explicitly added;
- distributed dispatch;
- distributed result merging;
- MessageBus;
- Slobrok;
- Config Server;
- Config Proxy;
- Java file distribution.

The supported product access paths are:

```text
Rust                 direct embedded API
non-Rust SDKs        gRPC carrying canonical FlatBuffer envelopes
browser/admin edge   optional browser-compatible gateway over the same contract
```

On Unix-like systems, local non-Rust clients should prefer a Unix-domain socket. TCP remains available where UDS is unavailable or deployment requires it.

The former portable client C ABI and per-language `NativeTransport` architecture are retired. The Rust ↔ Vespa C++ native boundary remains required and is unaffected.

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

**Roadmap state:** historical checkpoint; M12 and M13 extend this track.

Harden the complete M0–M10 `searchkernel-vespa-engine` surface independently of the portable client transport.

Cover:

- lifecycle races;
- malformed native inputs;
- native failures;
- repeated reopen;
- supported concurrency;
- memory safety;
- resource leaks;
- native sanitizers;
- long-running mutation/search workloads;
- M8 sorting/summaries/results;
- M9 grouping/facets/attribute mutation;
- M10 ranking/match features.

M11 does not depend on gRPC, language clients, or a portable client C ABI.

## M12 — Dense query tensors and nearest-neighbor execution

**Roadmap state:** implementation and acceptance contract are present; see
[current status](status.md) for the distinction between repository presence
and a passing verifier.

Freeze the first dense query-tensor and nearest-neighbor execution slice through
the real Vespa path. The v1 scope is literal dense tensors with one indexed
dimension, named query inputs, and exact nearest-neighbor execution. Sparse,
mixed, mapped, and expression query inputs remain outside this milestone.

The accepted paths are:

```text
low-level Rust API
    -> native adapter
    -> Vespa query/ranking properties and query tree
    -> Proton matcher

FlatBuffers
    -> searchkernel-ir
    -> searchkernel-ir-adapter
    -> the same low-level API and native path
```

The normative contract and cumulative verifier are in
[`reference/vespa/m12/`](../reference/vespa/m12/).

## M13 — Dense tensor document CRUD

**Roadmap state:** implementation and acceptance contract are present; M13 is
cumulative on M12.

Extend the real Vespa document and update paths to support whole-value dense
tensor document lifecycle behavior: insert/upsert, readback, assignment,
unset/reassign, close/reopen persistence, delete, and nearest-neighbor results
after an update. Tensor construction and validation remain in the native Vespa
adapter; Search Kernel does not add a parallel document store or tensor engine.

The normative contract and cumulative verifier are in
[`reference/vespa/m13/`](../reference/vespa/m13/).

---

# C-series — Portable API, Rust API, and RPC clients

The C-series remains orthogonal to the M-series.

Its final product surface is deliberately smaller than the previous native-client architecture:

```text
api/*.fbs                         canonical portable domain contract
searchkernel-ir                   portable semantics
searchkernel                      idiomatic direct Rust API
searchkernel-testkit              standalone semantic verification
searchkernel-rpc / searchkerneld  transport host
flatc                             language FlatBuffers model generation
standard gRPC generators          RPC stub generation
language packages                 thin ergonomic facades
```

The following are **not** final product components:

```text
portable client C ABI
searchkernel-ffi
per-language NativeTransport implementations
custom Go/Java/Node/PHP native bindings
custom universal transport abstraction
custom multi-language RPC stub generation
```

The C-series must remain runnable and verifiable without linking `searchkernel-vespa-engine` where the milestone concerns only portable semantics or transport framing. Recording and scripted backends are allowed for standalone acceptance, but must never become a production search engine.

The canonical semantic path is:

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

### Capability stages

Portable growth is intentionally staged:

```text
REPRESENTED  FlatBuffers can express it
SEMANTIC     *_to_ir() validates and resolves it
EXECUTABLE   Vespa integration implements it
ERGONOMIC    a high-level SDK offers convenient syntax
```

A feature does not need to advance through all four stages in the same change.

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
let runtime = IndexRuntime::open("./products")?;
let index = runtime.index();

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

## C5 — Historical portable client ABI — retired

C5 implemented the former byte-oriented portable C ABI used by first-party native language transports.

That architecture is superseded by gRPC and is **not** part of the final product runtime.

Do not extend C5 for new operations or languages.

After the gRPC path and migrated Python client are proven, remove:

```text
searchkernel-ffi
portable sk_call-style client ABI
portable handle/buffer ownership machinery
FFI conformance dedicated to that client boundary
language NativeTransport implementations
native client packaging built around that ABI
```

This retirement does **not** affect:

```text
searchkernel-vespa-bindings
Search Kernel C++ adapter ABI
Rust ↔ Vespa native integration
```

Historical C5 verifier artifacts may remain in repository history, but must not remain an active acceptance dependency after cutover.

---

## C6 — BFBS tooling — narrowed final scope

C6 established BFBS normalization and generation infrastructure. The final architecture keeps the parts that are Search Kernel-specific and removes the obligation to become a universal SDK compiler.

Standard tooling owns standard code generation:

```text
flatc                  → language FlatBuffers model types
standard gRPC tooling  → RPC client/server stubs
```

The following are no longer required bindgen responsibilities:

```text
custom generated transport abstraction
custom NativeTransport client core
per-language RPC plumbing
custom Rust/Python/Go/Java/Node/PHP transport emitters
```

Existing generated artifacts may remain temporarily during migration. Delete them once the replacement paths no longer depend on them.

C6 must continue to fail closed when BFBS contains schema features that Search Kernel-specific tooling claims to understand but cannot normalize safely.

---

## C7 — gRPC transport and `searchkerneld`

**Roadmap state:** implemented in the current repository. The daemon and
transport tests are the evidence; see [current status](status.md).

Implement the replacement non-Rust transport as a small, transport-only RPC layer.

### RPC contract

The `.proto` must contain no Search Kernel domain model. Conceptually:

```proto
syntax = "proto3";

package searchkernel.rpc.v1;

message Frame {
  bytes payload = 1;
}

service SearchKernel {
  rpc Execute(Frame) returns (Frame);
}
```

`Frame.payload` is a canonical FlatBuffers `RequestEnvelope` or `ResponseEnvelope`.

Do not add one protobuf RPC method per Search Kernel operation merely for convenience. Operation discovery remains owned by the FlatBuffers request/response unions.

### Server path

```text
gRPC Frame
    ↓
payload bytes
    ↓
FlatBuffer verifier
    ↓
canonical RequestEnvelope
    ↓
shared dispatcher
    ↓
*_to_ir()
    ↓
backend
    ↓
canonical ResponseEnvelope bytes
    ↓
gRPC Frame
```

### Transport behavior

Support:

```text
Unix-domain socket   preferred local path on Unix-like systems
TCP                  supported fallback / deployment option
```

Transport concerns may include:

```text
connection handling
deadlines/cancellation
message-size limits
socket addressing
transport-level errors
health/readiness
```

They must not include Search Kernel request semantics.

### Standalone verification

C7 acceptance may use C3 `RecordingBackend` and `ScriptedBackend`.

Verify at minimum:

- valid RequestEnvelope round trip;
- malformed FlatBuffer rejection;
- wrong root/envelope rejection;
- request/response size limits;
- UDS request/response path;
- TCP request/response path where supported;
- identical `*_to_ir()` output for direct portable input and RPC-carried input;
- gRPC transport errors remain distinct from portable Search Kernel errors.

C7 is implemented additively beside the old client path. Do not delete the old path until C8 proves the migrated user-facing client.

---

## C8 — Python gRPC client and cutover

**Roadmap state:** the gRPC transport and ergonomic facade are present in the
current repository. Treat deletion/acceptance claims separately from code
presence; see [current status](status.md).

The imperative migration steps below record the sequence that led to the
current implementation. They are not instructions to restore the retired
native client path; use [history](history.md) and [current status](status.md)
when deciding whether a cutover step remains outstanding.

Migrate the existing Python product API onto the new RPC architecture without redesigning its user-facing semantics.

### Low-level stack

```text
flatc-generated Python FlatBuffer models
        +
standard generated Python gRPC stub
        +
small Search Kernel transport wrapper
```

### Ergonomic facade

Preserve the useful high-level Python surface such as:

```text
Index
SearchBuilder
field()/FieldRef helpers
filters
ranking selection
result wrappers
portable error mapping
```

The Python facade may provide Python-specific syntax sugar. It must not independently implement Search Kernel defaults, field capability rules, ranking semantics, or request validation that belongs in `searchkernel-ir`.

### Verification and cutover

During migration, run the old and new Python paths side-by-side for the same portable scenarios.

Verify:

```text
same high-level call
    ↓
old path canonical meaning == new gRPC path canonical meaning
```

Once the gRPC path passes the portable Python corpus:

1. make gRPC the authoritative Python transport;
2. remove Python `NativeTransport` and native-library discovery/package logic;
3. remove the portable client FFI and its active verifier dependencies;
4. remove obsolete generated transport/client-core machinery no longer used by Rust or Python.

The deletion is part of C8 completion, not an optional cleanup milestone.

---

## C9 — Additional language facades on demand

Additional first-party language clients use the same architecture:

```text
idiomatic facade
    ↓
flatc-generated FlatBuffers models
    ↓
canonical RequestEnvelope bytes
    ↓
standard generated gRPC stub
```

Candidate languages include:

```text
Node.js / TypeScript
Go
Java
PHP
```

Do not add a custom native transport or Search Kernel-specific RPC generator for a new language.

A language becomes first-class only when there is a concrete product reason to support and package it. The roadmap does not require all candidate languages to be implemented before the core product can stabilize.

Each first-class package must prove:

- canonical request construction;
- canonical response interpretation;
- portable error mapping;
- UDS where the language/runtime supports the chosen local target syntax;
- TCP fallback;
- no language-specific semantic defaults.

---

## C10 — Portable cross-language conformance

Prove that every first-class non-Rust facade represents the same portable semantics.

Representative path:

```text
different language facades
        ↓
flatc-generated models
        ↓
canonical FlatBuffer envelopes
        ↓
gRPC
        ↓
shared dispatcher
        ↓
same *_to_ir()
        ↓
identical canonical IR
```

### Request-side invariant

Equivalent client operations must produce equivalent canonical IR.

Byte-for-byte FlatBuffer identity is not required unless a canonical byte encoding is explicitly promised.

### Response-side invariant

A canonical scripted response/error must produce equivalent language-level meaning in every first-class SDK.

### What C10 no longer tests

C10 does not maintain a native-vs-HTTP-vs-FFI transport matrix.

The portable client transport is gRPC. Rust is separately verified as the direct embedded API. Optional browser/JSON gateways have their own edge conformance and must still converge on the same FlatBuffers/IR semantics.

---

# I-series — Integration

The I-series is the only series that depends on both M and C artifacts.

The integration implementation belongs in:

```text
searchkernel-ir-adapter
```

The I-series translates already-frozen Search Kernel IR semantics into already-verified Vespa capabilities. It does not own client transport.

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

This is the first real implementation of `IndexRuntime::create` / `IndexRuntime::open` against Vespa.

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

## I9 — Rust and gRPC real-engine conformance

Run the same representative product scenarios through the two supported execution entry points:

```text
Rust direct embedded API

first-class non-Rust facade
    ↓
gRPC
    ↓
searchkerneld
```

against the real integrated Vespa backend.

A representative fixture should cover:

```text
create/open index
bulk insert
search
filter
facet/group where supported
sort
ranking/query inputs where supported
patch/update
search again
delete
close
reopen
search
```

Where vector/tensor support exists, include representative binary-heavy payloads.

Required invariant:

```text
same portable intent
    ↓
Rust direct path and gRPC path
    ↓
equivalent Search Kernel IR meaning
    ↓
equivalent real-engine outcome
```

I9 does not resurrect native foreign-language clients or a portable client FFI.

---

## I10 — Packaging and product hardening

Produce clean-machine packages for the supported product modes:

```text
Rust embedded library/package
searchkerneld daemon
selected first-class gRPC language SDKs
```

The target machine must not require:

```text
Vespa source checkout
Vespa development container
Vespa build tools
Vespa Java runtime/services
```

Record:

```text
Search Kernel version
portable FlatBuffers API version/BFBS identity
gRPC transport version
pinned Vespa revision
target platform
```

Then harden the complete product for:

```text
lifecycle races
malformed FlatBuffer payloads
malformed RPC frames
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
UDS RPC overhead
TCP RPC overhead where relevant
```

The benchmark goal is not to prove that RPC equals an in-process function call. It is to verify that the local RPC boundary is operationally acceptable for supported workloads and that serialization/transport overhead is measured rather than assumed.

---

# Parallel execution

The intended development shape is now:

```text
M TRACK

M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11


PORTABLE / CLIENT TRACK

C1 -> C2 -> C3 -> C4
              │
              ├── current FBS/IR parity widening
              │
              ▼
             C7 -> C8 -> C9 -> C10

C5 is retired.
C6 is retained only in its narrowed BFBS-tooling role.
```

Transport migration must not be mixed into the same agent task as a large FlatBuffers/IR redesign.

The preferred sequence is:

```text
1. finish/freeze the current query-language + FBS + IR widening
2. implement C7 gRPC beside the old path
3. prove C8 Python through gRPC
4. delete portable client FFI/native transports/obsolete client machinery
5. simplify bindgen to its surviving BFBS responsibilities
6. add more language facades only when needed
```

Integration trails whichever semantic/backend capabilities are ready:

```text
C1/C2 + M6
    -> I2 schema mapping

C4 + M2/M5
    -> I3 writes

C4 + M3/M4/M7
    -> I4 lexical search

C4 + M8
    -> I5 results

C4 + M9
    -> I6 facets/grouping

C4 + M10
    -> I7 ranking
```

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
did the caller produce the correct canonical FlatBuffer request?

did the request reach the same *_to_ir() implementation?

did equivalent language operations produce equivalent canonical IR?

did canonical responses produce equivalent language-level results/errors?
```

For C7/C8 additionally verify that gRPC is transport-only and does not introduce a second domain schema.

### Generation verification

The build should verify reproducible generated artifacts from the pinned toolchains:

```text
.fbs -> flatc language models / BFBS
.proto -> standard gRPC stubs
BFBS -> Search Kernel-specific metadata where retained
```

A schema change without regenerated checked-in artifacts, where artifacts are intentionally checked in, must fail CI.

### BFBS coverage

Search Kernel BFBS tooling must fail on schema features it claims to normalize but cannot represent safely.

It must not silently skip unknown fields, unions, attributes, references, or presence semantics.

### RPC envelope verification

Every RPC request must validate the FlatBuffer root/envelope before dispatch.

Malformed bytes, wrong roots, unsupported protocol versions, and oversized requests must fail before `*_to_ir()`.

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

Once a language package becomes first-class, a portable API change is complete for that language when:

```text
canonical .fbs updated
+
BFBS / flatc outputs refreshed as required
+
searchkernel-ir semantics updated
+
portable conformance passes
+
affected ergonomic facade tests pass
```

The gRPC `.proto` should normally remain unchanged when domain operations or fields are added.

Language syntax may differ idiomatically.

Semantic meaning may not drift.

There must never be independent definitions such as:

```text
Python defaults
Go defaults
PHP defaults
Java defaults
Node defaults
RPC defaults
```

for the same portable operation.

A newly represented portable capability does **not** require immediate ergonomic sugar in every language.

---

# Checkpoints

## After M4

The low-level track has proven real Rust-analysis → Vespa text indexing/matching.

## After C4

The high-level Rust product API is usable and testable without Vespa.

## After contract parity freeze

The widened query-language/FBS/IR contract can represent the intended Vespa request surface without forcing backend or SDK work in the same change.

## After C7

The replacement transport is proven:

```text
canonical FlatBuffer envelope
    ↓
gRPC Frame
    ↓
UDS/TCP
    ↓
searchkerneld
    ↓
same *_to_ir()
```

No domain semantics exist in protobuf.

## After C8

Python has migrated to gRPC with its existing ergonomic API preserved, and the portable client FFI/native-client path has been removed from the active product.

## After C10

Every first-class non-Rust SDK uses the same gRPC transport pattern and passes the same portable conformance corpus.

## After M8

The low-level wrapper can construct schemas/queries and execute useful local searches without the Vespa Java runtime.

## After I4/I5

The high-level API performs real lexical search through Vespa.

## After I9

Rust direct execution and first-class gRPC clients pass the same real-engine product scenarios.

---

# Vespa upgrades

Vespa remains pinned.

Updating Vespa is an explicit compatibility exercise, not routine dependency churn.

Every upgrade must verify:

- C++ adapter compatibility;
- the Rust ↔ Vespa native boundary;
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
- additional first-class language facades such as C#, Ruby, and Swift;
- browser-specific gRPC-Web/Connect or HTTP gateway work if required;
- streaming RPC only if a measured workload justifies it;
- dependency/runtime slimming.

A new language package should use standard FlatBuffers model generation plus standard gRPC stubs and a thin idiomatic facade.

Do not reintroduce a first-party per-language native transport without a concrete requirement that cannot be met by the daemon architecture.

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
standard FlatBuffers/gRPC generation
```

and:

```text
Search Kernel-specific transport/client code generation
```

prefer the standard tooling.

When widening the portable API, prefer:

```text
represent -> resolve semantics -> execute -> add ergonomic sugar
```

over forcing every layer to change in one milestone.

The desired end state is:

```text
one canonical FlatBuffers domain contract
one API → IR semantic implementation
one direct Rust product API
one small gRPC transport for non-Rust clients
one required Rust ↔ Vespa native boundary
thin language facades
```

not a matrix of native ABIs, transports, and independently generated client runtimes.
