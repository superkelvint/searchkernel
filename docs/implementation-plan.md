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
- reject raw FFI outside `searchkernel-vespa-bindings` and the portable FFI
  boundary;
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

```text
searchkernel2/
├── api/
│   └── *.fbs
├── core/
│   ├── crates/
│   │   ├── searchkernel-protocol/
│   │   ├── searchkernel-ir/
│   │   ├── searchkernel/
│   │   ├── searchkernel-ffi/
│   │   ├── searchkernel-testkit/
│   │   ├── searchkernel-client/
│   │   ├── searchkernel-json/
│   │   ├── searchkernel-vespa-engine/
│   │   ├── searchkernel-vespa-bindings/
│   │   └── searchkernel-ir-adapter/
│   └── native/
│       ├── vespa/
│       └── sdk/
├── clients/
│   └── README.md
├── searchkerneld/
│   └── README.md
├── searchkernel-web/
│   └── README.md
├── upstream/
│   └── vespa/
├── reference/
│   ├── api-conformance/
│   ├── clients/
│   ├── ffi-conformance/
│   ├── json-conformance/
│   └── vespa/
├── tools/
│   └── searchkernel-bindgen/
├── docs/
├── AGENTS.md
└── README.md
```

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

Harden `searchkernel-vespa-engine` independently.

Exercise:

```text
ASan / UBSan
lifecycle races
malformed input
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

M11 has no dependency on completion of the C-series.

---

# 9. C-series — High-Level API and Clients

The C-series has its own verifier hierarchy:

```text
reference/clients/cN/
    CONTRACT.md
    scenarios/
    expected/
    verify-cN.sh
```

Where API-shape compilation is relevant, also maintain language-specific compile/typecheck fixtures.

C-series acceptance never requires a working Vespa engine.

## C1 — Portable API contract

### Goal

Freeze the language-neutral semantic request/response model.

### Artifact

```text
api/*.fbs
```

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

The protobuf is canonical plumbing, not user-facing style.

It must not encode Rust-only concepts such as:

```text
lifetimes
trait bounds
AsRef<Path>
Rust enum memory layout
```

Every C-series public API operation must be representable losslessly in C1.

### Verify

- schema evolution compatibility tests;
- encode/decode round trips;
- canonical representation fixtures;
- stable field/tag numbering checks.

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
facet semantics
prefix/typo policies
portable errors
resolved requests
```

### Critical rule

Defaults are resolved here, not in individual SDKs or the Vespa adapter.

For example:

```text
Field::text("title")
```

resolves to a complete `ResolvedField` before reaching any backend.

### Verify

Golden inputs -> canonical resolved values.

No Vespa dependency is allowed.

---

## C3 — Standalone conformance testkit

### Goal

Allow the complete high-level/client surface to be developed without real search.

### Crate

```text
searchkernel-testkit
```

### RecordingBackend

Receives resolved operations and records them exactly.

It is used to prove:

```text
language API call
    ↓
correct portable/resolved request
```

### ScriptedBackend

Returns predefined responses/errors.

It is used to prove:

```text
portable response
    ↓
correct language result/error
```

### Forbidden behavior

The testkit must not implement:

```text
BM25
prefix matching
fuzzy matching
faceting
ranking
search execution
```

It is infrastructure for API verification, not another backend.

---

## C4 — Rust high-level API

### Goal

Implement the product-facing Rust API.

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

Compile fixtures plus C3 canonical-request/result scenarios.

The Rust package must still build without `searchkernel-vespa-engine`.

---

## C5 — Portable native ABI

### Goal

Create the universal in-process transport used by non-Rust clients.

### Crate

```text
searchkernel-ffi
```

### ABI shape

Prefer a small surface around:

```text
ABI version
open/create handle
call with request bytes
receive response bytes
close handle
free returned buffer
```

Only portable primitives cross the boundary:

```text
opaque handles
pointer + length byte buffers
integer status codes
explicit ownership
```

Do not expose Rust strings, Vecs, trait objects, callbacks, or language-GC objects through the universal ABI.

---

## C6 — Binding generator

### Goal

Automate the mechanical portions of language-client maintenance.

### Tool

```text
tools/searchkernel-bindgen
```

### Generate

```text
request/response models
enums/tagged unions
serialization glue
error mappings
documentation stubs
conformance fixtures
```

The generator must not force every language to mimic Rust method-chaining syntax.

Handwritten facades should remain small.

---

## C7 — Python client

### Goal

Deliver an idiomatic Python frontend to the C1 semantics.

### Verify

The canonical Python scenarios must produce the same resolved requests and response meanings as the Rust C4 client.

No Python-specific search semantics are allowed.

Real Vespa execution is deferred to I-series verification.

---

## C8 — Node.js / TypeScript client

### Goal

Deliver an idiomatic Node/TypeScript frontend.

TypeScript types should be generated or mechanically synchronized with C1.

### Verify

Run the same portable conformance scenarios as C4/C7.

No real Vespa dependency is required.

---

## C9 — PHP client

### Goal

Deliver an idiomatic PHP frontend.

The public API may use fluent PHP conventions, while request meaning remains C1.

A production package should prefer a normal extension/native package; raw PHP FFI is acceptable as a development path but not the semantic contract.

### Verify

Run the same portable conformance scenarios.

---

## C10 — Go client

### Goal

Deliver an idiomatic Go frontend.

Use Go conventions such as `context.Context`, explicit errors, and functional options where appropriate rather than mechanically reproducing Rust builders.

Embedded operation may rely on cgo.

### Verify

Run the same portable conformance scenarios.

---

# 10. I-series — Integration

The I-series is the only series that depends on both M and C artifacts. Its
acceptance artifacts belong under the appropriate `reference/` conformance
area when an integration milestone is added.

The integration implementation belongs in:

```text
searchkernel-ir-adapter
```

## I1 — Adapter skeleton and firewall

### Goal

Create the composition boundary without yet mapping significant functionality.

### Verify

Enforce the dependency rule:

```text
C-series crates -> no searchkernel-vespa-engine dependency
M-series crates -> no searchkernel-ir dependency
integration crate -> may depend on both
```

Also verify that no low-level Vespa types leak into the public high-level API.

---

## I2 — Index lifecycle and schema

### Dependencies

At minimum:

```text
C1/C2
M1/M6
```

### Goal

Compile `ResolvedIndexDefinition` into the low-level Vespa configuration and wire real `IndexRuntime::create/open`.

### Verify

For canonical schemas, inspect both:

```text
resolved high-level semantics
compiled Vespa configuration
```

then instantiate the real engine.

---

## I3 — Document operations

### Dependencies

At minimum:

```text
C1-C4
M2/M5
```

### Goal

Map:

```text
insert
upsert
patch
get
delete
bulk
```

onto the real low-level engine.

### Verify

Use high-level operations and confirm real Vespa state/search behavior.

No scripted backend is allowed in I-series acceptance.

---

## I4 — Lexical search and filters

### Dependencies

At minimum:

```text
C1-C4
M3/M4/M7
```

### Goal

Compile resolved high-level queries into low-level Vespa queries.

Cover:

```text
search text
search-field selection/weights
analysis
terms
phrases
AND/OR/NOT
exact filters
range filters
exists
```

### Verify

High-level request -> real Matcher -> high-level result.

---

## I5 — Results, sorting, projection, pagination, highlighting

### Dependencies

```text
C1-C4
M8
```

### Goal

Map the high-level result contract onto local Vespa search/docsums.

Cover:

```text
hits
totals
projection
sorting
limit/offset
highlighting/snippets where supported
```

---

## I6 — Facets and grouping

### Dependencies

```text
C1-C4
M9
```

### Goal

Compile high-level facet/group operations onto native grouping.

Keep facet semantics owned by `searchkernel-ir`.

---

## I7 — Ranking

### Dependencies

```text
C1-C4
M10
```

### Goal

Compile high-level ranking policies into supported Vespa ranking configuration/request inputs.

Initial policies:

```text
DefaultRelevanceV1
BM25
search-field weights
business tie-break fields
hard sorts
```

---

## I8 — Higher-level search semantics

### Goal

Integrate semantic conveniences that may compile into multiple Vespa primitives:

```text
prefix
fuzzy/typo tolerance
synonyms
presets
curations
facet-value search
```

The C-series specifies their behavior.

The M-series supplies the underlying capabilities.

The I-series owns only the translation.

---

## I9 — Cross-language real-engine conformance

### Dependencies

First-class C-series clients plus the required integrated product surface.

### Goal

Run one scenario corpus through:

```text
Rust
Python
Node.js / TypeScript
PHP
Go
```

against the real embedded engine.

### Canonical scenario

At minimum:

```text
create product index
bulk insert documents
search text
filter
facet
sort
patch document
search again
delete
close
reopen
search
```

All clients must have equivalent semantic outcomes.

---

## I10 — Packaging and full-product hardening

### Goal

Produce supported clean-machine packages and exercise the complete system.

The target machine must not require:

```text
Vespa source checkout
Vespa development container
Vespa build tools
Vespa Java runtime/services
```

Package at least the first-class clients selected for release.

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
```

---

# 11. Orthogonal sequencing rules

The intended post-M2 implementation schedule is:

```text
M TRACK

M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11


C TRACK

C1 -> C2 -> C3 -> C4 -> C5 -> C6 -> C7 -> C8 -> C9 -> C10
```

These arrows mean dependencies **inside the track only**.

There are no implicit diagonal dependencies.

For example, all of the following are valid states:

```text
M5 complete, C2 complete
M8 complete, C4 complete
M4 complete, C9 complete
C10 complete, M6 complete
```

Integration milestones are the only place where cross-track prerequisites are declared.

This is important for both human teams and coding agents: an M-series task should not start editing C-series code merely because it needs a convenient abstraction, and a C-series task should not import Vespa merely to make a standalone test pass.

---

# 12. Client parity rule

Once a client becomes first-class, a change to the portable API is not complete until:

```text
C1 contract updated
+
C2 semantics updated
+
generated bindings refreshed
+
all affected client conformance suites pass
```

Language syntax may differ idiomatically.

Semantic meaning may not drift.

Examples:

```text
Rust builder
Python kwargs/operators
TypeScript options
PHP fluent methods
Go functional options
```

may all represent the same C1 request.

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

The implementing agent must not modify earlier M-series contracts or verifiers to make new work pass.

## C-series milestone N

Complete only when:

```text
portable/client contract frozen
canonical expected request/result behavior frozen
implementation complete
all affected language compile/type checks pass
C-series conformance tests through CN pass
no Vespa dependency has leaked into standalone C-series crates
```

C-series completion does not require real search.

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

Reviewer focus:

```text
fake backend
fixture-specific logic
Rust reimplementation of Vespa behavior
Java runtime dependency sneaking into production
native library linked but not actually used
API-contract bypass
unsafe ownership shortcuts
modified verifier/goldens
```

## C-series

For every C milestone:

```text
1. freeze portable/high-level semantics
2. define canonical request/response fixtures
3. define language compile/type fixtures where relevant
4. review verifier and commit specification
5. implementation agent
6. parity reviewer
7. fixer
8. independent verifier
```

Reviewer focus:

```text
language-specific semantic drift
defaults duplicated in SDK
validation duplicated inconsistently
Rust-specific assumptions in portable API
Vespa dependency leakage
fake search implementation in testkit
handwritten generated-code divergence
```

## I-series

For every I milestone:

```text
1. identify previously committed C semantics
2. identify already-verified M capability
3. define translation contract
4. freeze end-to-end scenarios
5. implement adapter
6. review for semantic loss/leakage
7. run real-engine verifier
```

The integration agent does not redesign either public API.

---

# 15. Immediate work

M0–M2 are complete and remain unchanged.

The project can now start **two independent next milestones in parallel**:

```text
M3 — Linguistics compatibility
```

and:

```text
C1 — Portable API contract
```

M3's oracle comes from the pinned Vespa Java linguistics implementation.

C1 freezes the language-neutral product contract, including the protobuf representation required for later Rust/Python/Node/PHP/Go clients.

Neither task should depend on the other.

The first useful integration work can begin once its named prerequisites exist; there is no need to wait for either entire track to finish.
