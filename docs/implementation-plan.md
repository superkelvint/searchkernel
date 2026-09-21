# Implementation Plan

This plan starts from the repository as it exists now. Completed milestone
specifications are intentionally not repeated here. Historical contracts and
acceptance details stay under [`reference/vespa/`](../reference/vespa/); this
document describes only active implementation work and the rules for doing it.

## 1. Non-negotiable rules

- Keep one portable semantic path: FlatBuffers -> `searchkernel-ir` -> backend.
- Rust stays direct/in-process; non-Rust clients use the shared dispatcher over
  gRPC/gRPC-Web.
- Protobuf is transport framing only.
- Keep the only required native FFI at Rust <-> Vespa C++.
- Freeze expected behavior before implementing a new semantic capability.
- Real-engine acceptance must not use mocks, a Rust matcher, or fixture-specific
  production shortcuts.
- Normal Rust/client development must not require rebuilding Vespa.
- Generated FlatBuffers/gRPC code is owned by standard generators; do not grow a
  Search-Kernel-specific client generator.
- Do not implement grouping continuations in the current plan.

## 2. Baseline assumed complete

Do not spend implementation cycles redefining these items:

| Area | Baseline |
| --- | --- |
| Native engine | M0-M13 cumulative implementation/harness |
| Native/query expansion | Phase 9 |
| Public Rust real-engine path | Phase 10 + `reference/vespa/i` |
| Portable semantics | canonical V2 FlatBuffers, dispatcher, single search resolution path |
| Transport | `searchkerneld` UDS/TCP + gRPC-Web |
| Python | generated models + gRPC + managed local facade + E2E |
| TypeScript | generated models + browser gRPC-Web + managed Node facade + E2E |
| Search features | filters, typed queries, grouping/facets, YQL subset, ranking/two-phase, tensors/ANN, persistence |
| Docsums | default summary selection + scalar projected fields |

A task touching these areas is maintenance or extension, not completion of the
old milestone.

## 3. Pass 0 — make the current release state mechanically reproducible

Before adding another feature family, create one current-tree verification
entry point.

### Work

Create/extend a top-level verifier that runs, in a clean and isolated state:

```text
native M13 cumulative gate
Phase 9 specialized gate
I-series / Phase 10 product gate
SearchReply gate
recovery gate
workspace fmt/clippy/tests
code-generation drift checks
searchkerneld transport tests
Python unit/package/gRPC/E2E checks
TypeScript generate/typecheck/unit/package/managed-local E2E checks
```

Requirements:

- each native verifier uses an explicit SDK/build directory and does not depend
  on stale state from a previous run;
- generated-artifact checks fail with actionable drift diagnostics;
- the current capability ledger includes TypeScript and is checked against the
  actual supported surface rather than maintained as an unchecked narrative;
- `docs/status.md`, `docs/CAPABILITIES.md`, and this plan cannot claim an older
  state than the canonical E2E examples prove.

### Acceptance

One documented command succeeds from a clean checkout with the documented
prerequisites and leaves no hidden state requirement for the next run.

## 4. Pass 1 — finish result and docsum transport

Treat this as a sequence of small native/result passes, not one giant ABI edit.

### 1A. Rich docsum and summary configuration

Implement:

- custom `document_summaries` lowering into native schema/config;
- rich docsum values beyond the currently proven scalar path;
- explicit tests for default vs named summary classes and projection interaction.

Verify through native engine tests plus the public Rust path before adding SDK
sugar.

### 1B. Result feature fidelity

Extend the native result seam for:

- sort values;
- summary features;
- rank features where Vespa exposes them;
- typed tensor feature values rather than opaque bytes.

Do not derive these values from request state or recompute them in Rust.

### 1C. Grouped summary hits and highlighting

Implement the missing real-engine payloads for:

- grouping `summary()` -> child `Hit` nodes;
- highlight fragments/spans.

Then remove the explicit blocked/unsupported cases from Rust/Python/TypeScript
E2Es and replace them with positive assertions.

### Acceptance for Pass 1

- native ABI/result tests cover every newly exposed value kind;
- `reference/vespa/phase10` remains green;
- Python and TypeScript managed-local E2Es prove the same result semantics;
- no request-side feature is reported as supported while its required result
  payload is silently dropped.

## 5. Pass 2 — complete advanced document updates

Implement backend support in this order:

1. conditional insert/upsert/update/delete;
2. collection add/remove;
3. matched-element update;
4. tensor modify;
5. ordered multiple updates to the same target and richer field paths where the
   canonical IR can preserve Vespa ordering exactly.

For each operation:

```text
portable schema
-> *_to_ir semantic tests
-> adapter lowering
-> real native execution/persistence
-> public Rust facade
-> Python/TypeScript ergonomic parity
```

Do not add a facade operation before the canonical semantics are executable, and
do not approximate unsupported Vespa update semantics.

### Acceptance

Each operation has positive, negative, persistence/reopen, and cross-language
cases. Existing `Unsupported` errors are removed only when the real native path
is proven.

## 6. Pass 3 — expose the already-executable schema/query surface

Prefer exposing proven backend capability over adding new protocol variants.

### Schema facade work

Add high-level construction for:

- array-of-struct fields required by `sameElement`;
- position fields required by geo queries;
- predicate fields and their verified configuration subset.

Keep unsupported rich schema features explicit; do not imply full Vespa schema
model support.

### Query facade work

Close high-level helper gaps for already executable nodes, including the useful
subset of:

```text
near / ordered-near
alternatives
userInput / userQuery
nonEmpty
labels
IN / exists conveniences where missing
```

Also complete non-default search-field weighting once the ranking/native path
supports it.

### SDK parity work

- Rust: advanced UpdateBuilder parity, YQL/property conveniences, and missing
  grouping/query helpers that already have executable semantics.
- Python/TypeScript: fill typed-query/schema helpers where only the generated
  low-level API currently exposes the capability.

### Acceptance

Every public helper must lower to the same canonical IR as the equivalent
low-level FlatBuffers request. No SDK parser or semantic validator is introduced.

## 7. Pass 4 — cross-language product conformance

Create one shared real-engine corpus for the supported product surface.

Run the same scenarios through:

```text
Rust direct
Python gRPC/UDS
TypeScript Node gRPC/UDS
TypeScript browser gRPC-Web/TCP
```

At minimum cover:

```text
create/open
insert/upsert/get/update/delete
text + typed query
textual + structured filter
sort + pagination + projection
facet/group/aggregation
ranking + two-phase
scalar + tensor query inputs
nearest-neighbor/hybrid
summary/docsums
close/reopen
portable errors
coverage/degradation
```

Compare semantic outcomes, not language-specific object formatting.

### Acceptance

This corpus becomes part of the top-level verifier from Pass 0. A new first-class
SDK is not considered complete until it joins this gate.

## 8. Pass 5 — packaging, recovery, and measured hardening

After the semantic/result gaps above are closed:

- produce clean-machine packages for Rust, `searchkerneld`, Python, and
  TypeScript/Node;
- verify browser gRPC-Web deployment separately from local UDS mode;
- run crash/recovery at public product boundaries;
- run sanitizers and long mutation/search traces;
- measure startup, RSS/PSS, file descriptors, disk growth, indexing throughput,
  query latency, UDS overhead, TCP overhead, and gRPC-Web overhead;
- slim dependencies only where measurements or packaging constraints justify it.

Do not optimize by deleting verifier/oracle dependencies that still prove a
supported contract.

## 9. Later / demand-driven work

Add another language SDK only after a product need exists. The implementation
recipe is fixed:

```text
flatc models
+ standard gRPC stub
+ thin facade
+ shared conformance corpus
```

Potential richer Vespa capabilities such as sparse/mixed tensors, broader HNSW
configuration, richer schema inheritance/imports, ONNX/model assets, or broader
YQL/grouping syntax should each enter as their own verifier-first capability
slice.

Grouping continuations remain deliberately deferred and are not part of this
plan.

## 10. Definition of done for new work

A new capability is done only when all applicable layers are explicit:

```text
represented
semantic
executable
public/ergonomic where promised
verified against the real engine
covered by the relevant cross-language gate
```

Do not call a capability complete merely because the FlatBuffers type exists,
because an IR node exists, or because one SDK can construct it.
