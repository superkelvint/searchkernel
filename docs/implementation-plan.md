# Implementation Plan

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The final API architecture is Portable API (FlatBuffers) → `*_to_ir` in
`searchkernel-ir` → owned Search Kernel IR → backend interface → `lower_*`
Vespa representation → execution. Milestone labels in this document are
verification history, not architectural layers.

The implementation is deliberately incremental:

```text
define interface
    ↓
define expected behavior
    ↓
build oracle
    ↓
review verifier
    ↓
commit specification
    ↓
implement
    ↓
verify
```

An implementation agent does not define either the public interface or the expected behavior of the milestone it is implementing.

---

# 1. Rules

## 1.1 Wrap before abstracting

Prefer:

```text
existing Vespa capability
        ↓
small adapter
        ↓
idiomatic Rust representation
```

Do not invent a Search Kernel subsystem unless the underlying Vespa capability genuinely cannot provide what is needed.

---

## 1.2 Replace Java only where required

The embedded product deliberately removes most of Vespa's Java platform.

Replacement work is limited to functionality required to drive the retained C++ engine:

```text
document linguistics
query linguistics
query compilation
schema/config compilation
request/ranking property construction
```

Do not port:

```text
search chains
YQL
HTTP APIs
distributed dispatch
MessageBus
Slobrok
Config Server
Config Proxy
file distribution
```

unless a later milestone explicitly changes scope.

---

## 1.3 Check in the Rust API before implementation

Every milestone that adds public API must update the checked-in API contract
first. The verifier-authoring commit containing that contract is the
specification freeze boundary.

The harness contains compile-only programs such as:

```text
reference/vespa/api-contract/src/bin/mN.rs
```

These programs describe the exact API the implementation must provide.

An agent may implement the API.

It may not redesign it.

---

## 1.4 Freeze behavior before implementation

Each milestone is specified by:

```text
CONTRACT.md
scenarios/
expected/
oracle/
verify-mN.sh
```

Expected behavior is created before implementation starts.

The implementing agent must not regenerate expected data.

Review the verifier and expected outputs, then commit them as the frozen
specification before implementation begins. Git review protects that boundary.

---

## 1.5 Use the correct upstream oracle

For C++ capabilities:

```text
direct Vespa C++
        vs
Rust wrapper
```

For functionality being replaced from Java:

```text
Vespa Java implementation
        vs
Rust replacement
```

For end-to-end behavior:

```text
normal Vespa behavior
        vs
Search Kernel embedded path
```

---

## 1.6 No mock acceptance path

Mocks may be used in ordinary unit tests.

A milestone verifier must exercise the real path required by that milestone.

For example:

```text
M4 verification

Rust linguistics
    ↓
native ABI
    ↓
real Proton indexing
    ↓
real Matcher
```

A Rust-only text index does not satisfy M4.

---

## 1.7 Normal development must not require Docker

Docker is used to build or regenerate the native SDK and upstream oracles.

The normal development loop remains:

```bash
cargo test --workspace
```

An agent working on ordinary Rust code should not need to enter the Vespa build container.

---

## 1.8 Verification is cumulative

The acceptance command for M7 is conceptually:

```text
verify M0
verify M1
...
verify M7
```

Later milestones must preserve earlier behavior.

---

# 2. Milestone harness

Each milestone should contain:

```text
reference/vespa/mN/
    CONTRACT.md
    SOURCE_TRACE.md

    scenarios/
    expected/

    oracle/
    verify-mN.sh
```

Public API additions are also represented by:

```text
reference/vespa/API_CONTRACT.md
reference/vespa/api-contract/src/bin/mN.rs
```

## CONTRACT.md

Defines:

```text
required behavior
required execution path
explicit non-goals
forbidden shortcuts
```

## SOURCE_TRACE.md

Records the relevant pinned Vespa source path.

For Java-replacement milestones it also identifies the Java implementation being replaced.

Keep this short.

## scenarios/

Small deterministic inputs understandable by inspection.

## oracle/

Produces or documents authoritative upstream behavior.

## expected/

Frozen canonical outputs.

## verify-mN.sh

One acceptance command.

It must never regenerate its own expected results.

---

# 3. Anti-shortcut requirements

The verifier should check structure as well as output.

Where appropriate:

- verify native dynamic linkage;
- verify required native symbols are referenced;
- prevent the fake backend from being compiled into acceptance drivers;
- reject fixture IDs or values appearing in production source;
- reject Vespa headers outside the native adapter;
- reject raw FFI outside `searchkernel-vespa-bindings`; the retired portable client FFI must not reappear;
- compile the frozen API fixture;
- review `reference/vespa/` changes against the verifier/specification commit;
- run a separate reviewer looking specifically for bypasses.

For Java-replacement milestones, acceptance must prove the Rust implementation itself produced the required normalized representation.

Calling the Java oracle from production code does not count.

---

# 4. M0 — Native SDK

## Goal

Package the existing working embedded Vespa engine into:

```text
core/native/sdk/<target>/
```

usable outside the Vespa build environment.

## Required result

The SDK contains:

```text
include/
lib/
share/
manifest.json
```

and exposes the frozen native lifecycle ABI.

## Oracle

The existing successful embedded C++ prototype.

## Verify

Compile a small C program outside the Vespa source tree.

Exercise:

```text
open
close
open
close
```

Check runtime dependencies and ABI version.

## Forbidden shortcuts

- invoking the old gtest as the SDK;
- requiring the Vespa source tree;
- requiring the Vespa development container at runtime;
- exposing Vespa headers publicly.

---

# 5. M1 — Rust ownership

## Goal

Own the M0 engine safely from Rust.

## API

Use the checked-in M1 API.

## Verify

Compile the API contract and exercise:

```text
open
drop
open
drop
```

Also test invalid initialization.

Check that the executable actually links the Search Kernel native library.

## Forbidden shortcuts

- fake Rust engine;
- shelling out to another process;
- raw pointer in public API;
- bypassing the bindings/safe-wrapper layering.

---

# 6. M2 — CRUD and attribute search

## Goal

Drive the existing proven document mutation and attribute-search path through Rust.

## Scenario

```text
PUT doc1 attr1=10
PUT doc2 attr1=20

SEARCH 10
    -> doc1

UPDATE doc2 -> 10

SEARCH 10
    -> doc1, doc2

DELETE doc1

SEARCH 10
    -> doc2

SEARCH 20
    -> none
```

## Oracle

The preserved direct C++ embedded fixture.

## Verify

Compare canonical JSON produced by Rust against the frozen expected output.

Verify native mutation/search symbols are actually referenced.

## Forbidden shortcuts

- Rust document database;
- Rust-side integer matching;
- fixture-specific result generation;
- invoking the oracle as the implementation.

M0–M2 remain governed by the verifier package already created.

---

# 7. Development model after M2

M0–M2 are complete and retain their existing contracts and numbering.

From M2 onward, work is split into three milestone namespaces:

```text
M1, M2, M3, ...
    Embedded Vespa capability

C1, C2, C3, ...
    High-level/client platform

I1, I2, I3, ...
    Integration between the two
```

The important property is that the **M-series and C-series are orthogonal**.

Their numbers are local to their own series:

```text
C4 has no dependency on M4 merely because both contain 4.
M8 may complete before C5.
C8 may complete while M6 is still being implemented.
```

Each track is cumulative only within itself.

The I-series explicitly names cross-track dependencies when integration requires them.

## 7.1 Repository shape

Target final shape after the transport migration:

```text
searchkernel2/
├── api/
│   └── *.fbs
├── rpc/
│   └── searchkernel.proto
├── core/
│   ├── crates/
│   │   ├── searchkernel-protocol/
│   │   ├── searchkernel-ir/
│   │   ├── searchkernel/
│   │   ├── searchkernel-testkit/
│   │   ├── searchkernel-rpc/
│   │   ├── searchkernel-json/        # generic standard FlatBuffers JSON edge
│   │   ├── searchkernel-vespa-engine/
│   │   ├── searchkernel-vespa-bindings/
│   │   └── searchkernel-ir-adapter/
│   └── native/
│       ├── vespa/
│       └── sdk/
├── clients/
│   ├── python/
│   └── README.md
├── searchkerneld/
├── searchkernel-web/
├── upstream/
│   └── vespa/
├── reference/
│   ├── api-conformance/
│   ├── clients/
│   ├── rpc-conformance/
│   └── vespa/
├── tools/
│   └── generate-python-flatbuffers.sh
├── docs/
├── AGENTS.md
└── README.md
```

The following old client-platform components are removed after gRPC cutover:

```text
searchkernel-ffi
portable client C ABI
NativeTransport implementations
portable native FFI conformance
custom generated transport/client core where no longer used
```

The required Rust ↔ Vespa native boundary under `searchkernel-vespa-bindings` remains.

## 7.2 Dependency firewall

During standalone development:

```text
C-series

searchkernel
    ↓
searchkernel-ir
    ↓
searchkernel-protocol

NO dependency on searchkernel-vespa-engine
```

and:

```text
M-series

searchkernel-vespa-engine
    ↓
searchkernel-vespa-bindings
    ↓
native ABI / Vespa C++

NO dependency on searchkernel-ir
```

Only:

```text
searchkernel-ir-adapter
```

may depend on both.

## 7.3 Two public APIs remain separate

The C-series owns Search Kernel product concepts:

```text
Index
IndexDefinition
Field
Analyzer
SearchField
Patch
Filter
Ranking
Sort
Facet
SearchResult
```

The M-series owns the lower-level Vespa-facing interface:

```text
Engine
Vespa configuration/schema representation
native documents and updates
Vespa query representation
rank profiles
query features
native search results
```

Do not expose an `index.vespa()` escape hatch from the product API.

Applications that deliberately want the low-level API use `searchkernel-vespa-engine` directly.

---

# 8. M-series — Embedded Vespa Wrapper

M3 onward continues the low-level sequence established by M0–M2.

Its acceptance harness remains under:

```text
reference/vespa/mN/
```

and remains cumulative with M0–M2.

## M3 — Linguistics compatibility

### Goal

Implement the minimum linguistic preprocessing currently supplied outside the C++ search core.

This applies to both documents and queries.

### Initial scope

Record and reproduce:

```text
input string
language
tokens
normalized forms
stemmed/normalized terms
positions / spans where required
```

Index-time output must be sufficient to construct the linguistic annotations consumed by Vespa's C++ indexing path.

Query-time output must produce compatible terms.

### Oracle

Use the pinned Vespa Java linguistics implementation.

### Verify

Cover at least:

```text
case normalization
punctuation
multiple whitespace
Unicode text
accented text
stemming
empty input
repeated terms
positions
```

Then verify that real indexing/query integration consumes those results in the following milestones.

### Forbidden shortcuts

- toy ASCII tokenizer as production behavior;
- calling Java from the runtime;
- divergent index/query analysis without an explicit Vespa-equivalent reason.

---

## M4 — Text indexing and matching

### Goal

Connect M3 to the real C++ text index and Matcher.

Continue using the known precompiled text configuration fixture.

### Scenario

```text
doc1 = "The quick brown fox"
doc2 = "A slow red fox"

term brown
phrase "brown fox"

update doc2
verify old terms disappear

delete doc1
verify its postings disappear
```

Include BM25.

### Acceptance path

```text
Rust document
    ↓
M3 analyzer
    ↓
native document annotations
    ↓
Vespa index
    ↓
M3 query analysis
    ↓
Vespa Matcher
```

### Forbidden shortcuts

- Rust-side matching;
- Rust-side BM25;
- bypassing M3;
- retaining the spike tokenizer.

---

## M5 — Persistence and reopen

### Goal

Verify the wrapped engine as durable local state.

### Scenarios

Process A:

```text
open
put
update
delete
search
close
```

Process B:

```text
open same directory
search
update
search
close
```

Add controlled abrupt termination where practical.

### Persistent compatibility fixture

Record:

```text
Search Kernel revision
native ABI version
Vespa revision
fixture definition
expected results
```

Preserve selected data directories for future upgrade testing.

---

## M6 — Vespa schema/configuration

### Goal

Stop depending on precompiled configuration fixtures for the supported low-level schema subset.

This milestone defines the representation needed by `searchkernel-vespa-engine`, not the C-series product schema.

### Initial concepts

```text
text
keyword/string
integer
floating point
boolean where straightforward

index
attribute
stored/summary
```

### Implementation boundary

```text
searchkernel-vespa-engine config model
    ↓
embedded compiler
    ↓
configuration consumed by Proton
```

Use Vespa Java config-model as an oracle for selected schemas and validate resulting engine behavior.

### Forbidden shortcuts

- Java config-model at runtime;
- shipping fixture-specific generated config as production behavior;
- `.sd` parser;
- high-level product semantics in this crate.

---

## M7 — Vespa query compilation

### Goal

Compile typed low-level queries into the representation consumed by the C++ Matcher.

### Operators

```text
Term
Phrase
And
Or
Not
Equals
Range
Exists
```

### Processing

```text
searchkernel-vespa-engine Query
    ↓
M6 field metadata
    ↓
M3 text analysis where required
    ↓
Vespa query representation
```

### Verify

Cover:

```text
single term
phrase
nested AND/OR
NOT
text + numeric filter
range
unknown field
wrong value type
```

Then execute through the real Matcher.

### Forbidden shortcuts

- YQL parser;
- generic query language;
- Rust-side matching;
- treating every field as text.

---

## M8 — Local search request and results

### Goal

Complete the ordinary local request/result path.

### Scope

```text
limit
offset where supported
sorting
stored/summary fields
scores
total hits available from the engine
```

Use M7 for matching/filtering.

Wrap the native docsums/summary path where needed.

No distributed fill phase exists in this embedded scope.

---

## M9 — Attributes, grouping, and facets

### Goal

Broaden mutable attribute functionality.

### Scope

```text
attribute field types
updates
filters
sorting
ranking access
group/facet counts
basic aggregation
```

Use real Vespa capabilities rather than a Rust aggregation engine.

---

## M10 — Ranking integration

### Goal

Expose the Vespa ranking capabilities needed by the embedded low-level API.

### Initial scope

```text
BM25
field-specific ranking
attribute features
query features
rank properties
ranking expressions
feature output
```

Ranking evaluation remains C++.

### Forbidden shortcuts

- Rust scoring engine;
- RankScript;
- manually computed production BM25/ranking.

---

## M11 — Low-level hardening

### Goal

Harden `searchkernel-vespa-engine` independently of the client transport architecture.

Exercise:

```text
ASan / UBSan
lifecycle races
malformed native input
native failure
repeated reopen
supported concurrency
long-running mutation/search traces
RSS/PSS
threads
file descriptors
disk growth
native errors
```

M11 has no dependency on the retired portable client FFI or on gRPC.

---

# 9. C-series — High-Level API and RPC Clients

The C-series verifier hierarchy remains:

```text
reference/clients/cN/
    CONTRACT.md
    scenarios/
    expected/
    verify-cN.sh
```

Where language API shape matters, maintain compile/typecheck fixtures.

C-series acceptance does not require a working Vespa engine unless a milestone explicitly moves into I-series integration.

The final client architecture is:

```text
Rust
    direct high-level API

non-Rust
    idiomatic facade
        ↓
    flatc-generated FlatBuffers model types
        ↓
    canonical RequestEnvelope bytes
        ↓
    standard generated gRPC stub
        ↓
    searchkerneld
        ↓
    shared dispatcher
        ↓
    searchkernel-ir
```

The portable client C ABI and first-party per-language native transports are retired.

## C1 — Portable API contract

### Goal

Freeze the language-neutral semantic request/response model.

### Artifact

```text
api/*.fbs
```

FlatBuffers is the canonical domain contract.

### Initial messages

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

Error
```

### Rules

The `.fbs` schema must not encode Rust-only concepts such as:

```text
lifetimes
trait bounds
AsRef<Path>
Rust enum memory layout
```

Every portable operation must be representable losslessly in C1.

The gRPC `.proto` is **not** part of the Search Kernel domain model. It is transport framing only.

### Verify

- schema evolution compatibility checks;
- encode/decode round trips;
- canonical representation fixtures;
- BFBS reproducibility;
- stable identifiers/discriminators where compatibility requires them.

---

## C2 — High-level semantic core

### Goal

Implement Search Kernel product semantics without a backend.

### Crate

```text
searchkernel-ir
```

### Responsibilities

```text
stable defaults
schema validation
normalization
resolved field definitions
analyzer identity/version
filter type checking
ranking policies
facet/grouping semantics
portable errors
resolved requests
```

### Critical rule

Defaults and portable validation are resolved here, not in individual SDKs, gRPC handlers, or the Vespa adapter.

### Verify

Golden portable inputs -> canonical IR.

No Vespa dependency is allowed.

---

## C3 — Standalone conformance testkit

### Goal

Allow the complete portable/client surface to be developed without real search.

### Crate

```text
searchkernel-testkit
```

### RecordingBackend

Records resolved operations exactly.

Used to prove:

```text
client/API request
    ↓
correct Search Kernel IR
```

### ScriptedBackend

Returns predefined portable responses/errors.

Used to prove response mapping without implementing search.

### Forbidden behavior

The testkit must not grow BM25, query matching, grouping, ranking, fuzzy search, faceting, or another production search engine.

---

## C4 — Rust high-level API

### Goal

Maintain the ergonomic direct Rust product API.

Rust remains in-process and does not call gRPC merely to reach the local engine.

### Target surface

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

### Example

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

### Verify

C3 only.

---

## C5 — Historical portable client ABI — retire

### Status

Superseded by the gRPC architecture.

The old `searchkernel-ffi` byte-oriented client ABI is not part of the final runtime.

Do not extend it.

Its only role during migration is to keep the existing Python/native path working until C8 cutover.

After C8 equivalence passes, delete:

```text
searchkernel-ffi
sk_call-style portable client ABI
portable handles/buffers/status layer
portable FFI conformance as an active suite
Python NativeTransport
future cgo/JNI/N-API client plans
native client packaging built around C5
```

Do not delete or weaken `searchkernel-vespa-bindings`; that is the separate required Rust ↔ C++ engine boundary.

---

## C6 — BFBS tooling — narrow, do not expand

### Goal

Use official FlatBuffers tooling for schema generation and JSON parsing/printing.
There is no Search-Kernel-specific schema emitter. The checked-in BFBS files are
generated from `api/*.fbs`, and `searchkernel-json` consumes them generically.

### Stop owning

```text
universal generated client core
generic Transport / AsyncTransport abstraction
per-language native transports
custom gRPC stub generation
custom language wire-model generation already provided by flatc
```

### Verify

- deterministic normalization;
- malformed/orphan diagnostics;
- schema mutation coverage for invariants Search Kernel actually owns;
- no stale generated client artifacts remain required after C8 cleanup.

---

## C7 — gRPC envelope and daemon

### Goal

Build the replacement non-Rust transport beside the old path.

### Transport schema

Add:

```text
rpc/searchkernel.proto
```

with a deliberately tiny contract, conceptually:

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

No Search Kernel domain fields belong in protobuf.

### Runtime crate

Implement a small reusable RPC service layer, for example:

```text
searchkernel-rpc
```

`searchkerneld` owns process configuration and hosts that service.

### Server execution path

```text
Frame.payload
    ↓
verify FlatBuffer root/envelope
    ↓
shared canonical dispatcher
    ↓
*_to_ir()
    ↓
backend
    ↓
canonical ResponseEnvelope bytes
```

### Required transport support

```text
Unix-domain socket on supported Unix-like systems
TCP fallback / explicit network mode
```

### Verify

With `RecordingBackend` / `ScriptedBackend`:

- UDS request/response;
- TCP request/response where supported;
- malformed FlatBuffer rejection;
- wrong root/envelope rejection;
- unsupported protocol version rejection;
- request/response size limits;
- portable errors remain portable payload errors;
- gRPC failures remain transport errors;
- direct portable request and gRPC-carried request produce identical IR.

### Forbidden shortcuts

- protobuf domain model;
- one protobuf request type per Search Kernel operation;
- gRPC handler-specific defaults or validation;
- direct gRPC → Vespa semantic path bypassing `*_to_ir()`.

---

## C8 — Python gRPC migration and old-path deletion

### Goal

Move the existing ergonomic Python API to gRPC without changing its Search Kernel semantics.

### Build first: low-level proof

Before touching the facade, prove:

```text
flatc-generated Python models
    ↓
RequestEnvelope bytes
    ↓
generated gRPC stub
    ↓
UDS
    ↓
Rust searchkerneld
    ↓
RecordingBackend / ScriptedBackend
```

### Then port the facade

Preserve user-facing concepts such as:

```text
Index
SearchBuilder
field()/FieldRef
filter operators
ranking selection
result wrappers
```

The facade owns syntax sugar only.

### Equivalence gate

Temporarily run old and new Python paths against the same canonical scenarios.

Require equivalent canonical request meaning and response meaning.

### Cutover gate

C8 is not complete until the old client transport is deleted:

```text
Python NativeTransport removed
portable client FFI removed
portable FFI verifier removed from active CI
obsolete generated transport/client-core code removed
native-client packaging removed
```

The Vespa C++ native boundary remains untouched.

---

## C9 — Additional language facades on demand

### Goal

Add languages only when there is a concrete product need.

Candidates:

```text
Node.js / TypeScript
Go
Java
PHP
```

Each uses:

```text
flatc-generated domain models
+
standard generated gRPC stub
+
small idiomatic facade
```

No new language may require a Search Kernel-specific native transport or custom RPC generator as a condition of first-class support.

### Verify

Run the shared portable conformance scenarios.

Use language-native compile/type checks where applicable.

---

## C10 — Cross-language portable conformance

### Goal

Prove equivalent portable semantics across every first-class language facade.

### Request invariant

```text
equivalent language call
    ↓
canonical FlatBuffer request
    ↓
gRPC/shared dispatcher where non-Rust
    ↓
identical Search Kernel IR
```

### Response invariant

A canonical scripted response/error must produce equivalent language-level meaning in all first-class clients.

C10 does not maintain a native-vs-HTTP-vs-FFI matrix.

Rust direct and non-Rust gRPC are the two product entry patterns.

---

# 10. I-series — Integration

The I-series is the only series that depends on both M and C artifacts.

The integration implementation belongs in:

```text
searchkernel-ir-adapter
```

Transport does not belong in the I-series.

## I1 — Adapter skeleton and firewall

### Goal

Create the composition boundary without mapping significant functionality.

### Verify

```text
C-series crates -> no searchkernel-vespa-engine dependency
M-series crates -> no searchkernel-ir dependency
integration crate -> may depend on both
```

No low-level Vespa types leak into the public high-level API.

---

## I2 — Index lifecycle and schema

### Dependencies

```text
C1/C2
M1/M6
```

### Goal

Compile `ResolvedIndexDefinition` into low-level Vespa configuration and wire real `IndexRuntime::create/open`.

---

## I3 — Document operations

### Dependencies

```text
C1-C4
M2/M5
```

### Goal

Map insert/upsert/patch/get/delete/bulk onto the real engine.

No scripted backend is allowed in I-series acceptance.

---

## I4 — Lexical search and filters

### Dependencies

```text
C1-C4
M3/M4/M7
```

### Goal

Compile resolved high-level queries into low-level Vespa queries and execute the real Matcher.

---

## I5 — Results, sorting, projection, pagination, highlighting

### Dependencies

```text
C1-C4
M8
```

### Goal

Map the high-level result contract onto local Vespa search/docsums.

---

## I6 — Facets and grouping

### Dependencies

```text
C1-C4
M9
```

### Goal

Compile high-level facet/group operations onto native grouping while keeping portable semantics in `searchkernel-ir`.

---

## I7 — Ranking

### Dependencies

```text
C1-C4
M10
```

### Goal

Compile high-level ranking policies, rank-profile selection, query inputs/features, and other supported ranking request state into Vespa.

---

## I8 — Higher-level search semantics

### Goal

Integrate any Search Kernel-owned conveniences that compile into multiple Vespa primitives.

The C-series specifies their behavior; the M-series supplies capabilities; I-series owns only translation.

---

## I9 — Rust/direct vs gRPC real-engine conformance

### Dependencies

Integrated product surface plus C7/C8 and any additional first-class C9 clients under test.

### Goal

Run the same representative product corpus through:

```text
Rust direct embedded API
Python gRPC
additional first-class gRPC facades where available
```

against real Vespa.

### Canonical scenario

At minimum:

```text
create/open product index
bulk insert documents
search text
filter
facet/group where supported
sort
ranking/query inputs where supported
patch document
search again
delete
close
reopen
search
```

All entry paths must have equivalent semantic outcomes.

---

## I10 — Packaging and full-product hardening

### Goal

Produce supported clean-machine packages for:

```text
Rust embedded
searchkerneld
selected first-class gRPC language SDKs
```

The target machine must not require:

```text
Vespa source checkout
Vespa development container
Vespa build tools
Vespa Java runtime/services
```

Then run:

```text
sanitizers
state-machine tests
crash/recovery
supported concurrency
long-running workloads
resource/leak tests
package-size measurements
startup measurements
UDS RPC overhead measurements
TCP RPC overhead measurements where relevant
```

---

# 11. Orthogonal sequencing rules

The intended sequencing is:

```text
M TRACK
M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11

C CONTRACT/API
C1 -> C2 -> C3 -> C4

TRANSPORT RESET
finish/freeze current FBS+IR widening
    -> C7
    -> C8
    -> delete C5/native-client path
    -> narrow C6 to surviving BFBS responsibilities
    -> C9/C10 on demand
```

Do not combine the current large FBS/IR parity expansion and the transport rewrite in one implementation task.

There are no implicit diagonal dependencies between M and C. I-series milestones state their own cross-track prerequisites.

---

# 12. Client parity rule

A portable feature progresses independently through:

```text
represented in FlatBuffers
resolved in Search Kernel IR
executable by the backend
ergonomic in a facade
```

Once a language facade becomes first-class, it may lag in ergonomic sugar, but it must always be possible to represent supported low-level portable requests through the generated FlatBuffers model layer.

Defaults and validation are never independently redefined by each SDK.

---

# 13. Definition of done

## M-series milestone N

Complete only when:

```text
M-series API contract frozen where relevant
oracle frozen
expected outputs frozen
implementation complete
native build succeeds
fmt/clippy/tests pass
verify M0 ... verify MN pass
```

## C-series milestone N

Complete only when:

```text
portable/client contract frozen
canonical expected request/result behavior frozen
implementation complete
required generated artifacts reproducible
relevant language compile/type checks pass
C-series verifier passes
no Vespa dependency leaked into standalone semantic/client tests
```

For C7/C8 specifically, transport code must remain semantic-free and C8 must delete the retired portable client FFI path.

## I-series milestone N

Complete only when:

```text
required C-series prerequisites pass independently
required M-series prerequisites pass independently
adapter implementation complete
real Vespa path exercised
integration verifier passes
no scripted/recording backend participates in acceptance
```

---

# 14. Agent workflow

## M-series

For every M milestone:

```text
1. inspect pinned upstream source
2. define/freeze low-level API
3. build upstream oracle
4. define scenarios
5. generate expected outputs
6. build verifier
7. review verifier and commit specification
8. implementation agent
9. shortcut-focused reviewer
10. fixer
11. independent verifier
```

## C-series

For contract/semantic work:

```text
1. freeze portable semantics
2. define canonical request/response fixtures
3. review verifier and commit specification
4. implementation agent
5. parity reviewer
6. fixer
7. independent verifier
```

For C7/C8 transport migration, use additive tasks only:

```text
1. add tiny .proto and generated Rust/Python stubs
2. add standalone gRPC service around existing dispatcher
3. prove low-level Python FlatBuffer round trip over UDS
4. port Python facade without deleting old path
5. run old/new equivalence corpus
6. cut over
7. delete portable FFI/native path
8. simplify bindgen only after deletion
```

Do **not** ask an implementation agent to "refactor C5/C6/C7 to gRPC" in one pass.

Reviewer focus:

```text
second domain schema sneaking into protobuf
transport-specific defaults/validation
language semantic drift
old FFI kept alive through compatibility shims
custom generator growing where flatc/gRPC tooling already suffices
Vespa dependency leakage
```

## I-series

For every I milestone:

```text
1. identify committed C semantics
2. identify verified M capability
3. define translation contract
4. freeze end-to-end scenarios
5. implement adapter
6. review for semantic loss/leakage
7. run real-engine verifier
```

The integration agent does not redesign either public API.

---

# 15. Immediate work

The current priority is to reduce simultaneous moving parts.

Execute in this order:

```text
1. Finish and freeze the current query-language / FlatBuffers / IR widening.
   - represent the desired Vespa surface
   - complete *_to_ir() semantics
   - freeze positive/negative conformance cases
   - do not require all backend lowering or SDK sugar yet

2. Implement C7 as a greenfield additive RPC path.
   - tiny bytes-only .proto
   - Rust gRPC service
   - UDS first, TCP fallback
   - existing canonical dispatcher / *_to_ir()
   - no deletion of old client path yet

3. Prove one deliberately low-level Python request over gRPC.

4. Port the existing Python ergonomic facade to the gRPC path.

5. Run old-vs-new Python canonical equivalence.

6. Cut over and delete:
   - searchkernel-ffi / portable client ABI
   - Python NativeTransport
   - portable FFI conformance from active CI
   - obsolete generated client/transport machinery
   - native-client packaging/plans

7. Keep schema generation and JSON conversion on the official FlatBuffers path.

8. Add further language facades only when product demand requires them.
```

The transport migration must treat the frozen FlatBuffers contract and `searchkernel-ir` semantics as inputs, not redesign them while moving plumbing.
