# Implementation Plan

This plan starts from the repository as it exists now. Completed milestone
specifications are intentionally not repeated here. Historical contracts and
acceptance details remain under [`reference/vespa/`](../reference/vespa/).

The purpose of this document is to control what is allowed to move next.

## 1. Non-negotiable rules

- Keep one portable semantic path: FlatBuffers -> `searchkernel-ir` -> backend.
- Rust stays direct/in-process; non-Rust clients use the shared dispatcher over
  gRPC/gRPC-Web.
- Protobuf is transport framing only.
- Keep the only required native FFI at Rust <-> Vespa C++.
- Freeze expected behavior before implementing a new semantic capability.
- When a bug is reported, add a regression test first, then fix it.
- Real-engine acceptance must not use mocks, a Rust matcher, or
  fixture-specific production shortcuts.
- Normal Rust/client development must not require rebuilding Vespa.
- Generated FlatBuffers/gRPC code is owned by standard generators.
- Do not implement grouping continuations in the current plan.
- Do not treat implementation presence as acceptance.
- Prefer one active frontier. Park unrelated valid work instead of starting it
  in parallel.

## 2. Baseline present in the tree

These areas are implementation baseline, not invitations to redesign old
milestones:

| Area | Baseline present |
| --- | --- |
| Native engine | M0-M14 implementation surface and harness |
| Native/query expansion | Phase 9 |
| Public Rust real-engine path | Phase 10 + `reference/vespa/i` |
| Portable semantics | canonical V2 FlatBuffers, dispatcher, shared search-resolution architecture |
| Transport | `searchkerneld` UDS/TCP + gRPC-Web |
| Python | generated models + gRPC + managed local facade + E2E |
| TypeScript | generated models + browser gRPC-Web + managed Node facade + E2E |
| Search features | filters, typed queries, grouping/facets, YQL subset, ranking/two-phase, tensors, nearest-neighbor |
| Vector search | exact dense NN plus the M14 HNSW implementation slice |
| Results | scalar projected docsums plus first real highlighting path |

Whether each row is CLOSED is determined by its verifier/acceptance evidence,
not by this table.

## 3. Pass 0 — close the current semantic/schema frontier

Do this before adding another broad capability family.

### 0A. Schema fidelity

Use
[`implementation-plan/schema-fidelity-plan.md`](../implementation-plan/schema-fidelity-plan.md)
as the detailed checklist.

Required invariant:

```text
IndexDefinitionIr
    -> public Rust representation / describe
    -> IndexDefinitionIr
== original semantic state
```

Requirements:

- every checked implementation item has focused regression coverage;
- `definition_to_ir()` and `definition_from_ir()` preserve the promised
  canonical schema state;
- `Index::describe()` does not silently lose schema/ranking information;
- modifying a described public definition re-resolves from public builder state
  rather than accidentally retaining stale canonical state;
- unsupported Vespa execution remains explicit `Unsupported` and is not
  confused with schema representation fidelity.

### 0B. Canonical search resolution and typed queries

Prove that Rust and portable ingress converge before resolved search IR exists.

Requirements:

- one canonical unresolved search representation;
- one canonical resolver;
- no public/ungated constructor that permits a second resolved-search path;
- typed-query annotations, ranking state, query inputs, grouping, search fields,
  filters, sorting, and presentation controls all pass through the shared
  semantic resolution boundary;
- focused typed-query tests remain green;
- structural IR-boundary checks are green.

Do not widen the typed-query surface while fixing boundary correctness.

### 0C. Phase 9 / additive / YQL / grouping closure

Re-run and record the real acceptance state for the already-implemented work.

Verify:

- portable summary-hit semantics and exact IDs;
- additive ABI/query behavior;
- YQL/additive combination rules;
- grouping interactions;
- specialized Phase 9 verifier;
- cumulative verifier through the intended phase on the actual native machine.

A historical pass is not sufficient if relevant code changed afterward.

### 0D. Highlighting closure

The first supported slice is Vespa-generated dynamic teaser output for stored,
indexed text fields.

Acceptance must prove:

- real Vespa-generated fragment text;
- match spans, including linguistically analyzed/stemmed matches;
- projection interaction;
- Rust/Python/TypeScript behavior where the feature is promised.

Keep custom summary definitions, arbitrary Juniper controls, grouped highlights,
and rich docsum values separate.

### 0E. M14 HNSW closure

Use the frozen
[`reference/vespa/m14/CONTRACT.md`](../reference/vespa/m14/CONTRACT.md).

Acceptance must prove:

- native HNSW tensor attribute configuration is emitted;
- Euclidean distance metric is honored for the frozen fixture;
- exact search remains valid on an HNSW-enabled field;
- approximate search reaches Vespa HNSW;
- explore-additional-hits and distance-threshold controls reach execution;
- vector update changes ANN results;
- close/reopen preserves graph behavior;
- delete/reopen preserves deletion;
- portable HNSW schema lowers through the canonical adapter;
- no Rust-side ANN fallback, shadow graph, or exact-scan substitution.

### 0F. Capability/documentation truth

Before closing Pass 0:

- `tools/check-capabilities.py` is green;
- `LIMITATIONS.md`, `docs/CAPABILITIES.md`, `docs/status.md`,
  `docs/roadmap.md`, and this plan do not contradict current executable
  behavior;
- the CTO ledger records which workstreams are CLOSED and the evidence used.

### Acceptance for Pass 0

Pass 0 is complete only when:

- all concrete semantic/schema blockers are cleared;
- required unit/regression tests pass;
- Phase 9 and M14 real-engine verifier results are recorded;
- no second search semantic path remains;
- capability documentation is current;
- blocker-focused review finds no unresolved correctness blocker.

## 4. Pass 1 — make the release state mechanically reproducible

Create one clean-tree verification entry point.

Run, in a clean isolated state:

```text
native cumulative gate through M14
Phase 9 specialized gate
I-series / Phase 10 product gate
SearchReply gate
recovery gate
workspace fmt/clippy/tests
code-generation drift checks
searchkerneld transport tests
Python unit/package/gRPC/E2E checks
TypeScript generate/typecheck/unit/package/managed-local E2E checks
capability/status documentation checks
```

Requirements:

- explicit SDK/build directories for native verifiers;
- no dependence on stale prior-run state;
- actionable generated-artifact drift diagnostics;
- TypeScript included in the capability/release ledger;
- one documented command from a clean checkout.

## 5. Pass 2 — finish result and docsum transport

Treat this as small native/result slices.

### 2A. Rich docsum and summary configuration

Implement:

- custom `document_summaries` lowering into native schema/config;
- rich docsum values beyond the current scalar path;
- default vs named summary-class and projection interactions.

### 2B. Result feature fidelity

Extend the native result seam for:

- sort values;
- summary features;
- rank features where Vespa exposes them;
- typed tensor feature values rather than opaque bytes.

Do not derive these values from request state or recompute them in Rust.

### 2C. Grouped summary hits

Complete grouping `summary()` -> child `Hit` payloads through the native
result path.

### Acceptance for Pass 2

- native ABI/result tests cover every new value kind;
- public Rust path is proven;
- Python and TypeScript managed-local E2Es prove equivalent semantics;
- no supported request-side feature silently drops required result data.

## 6. Pass 3 — complete advanced document updates

Implement backend support in this order:

1. conditional insert/upsert/update/delete;
2. collection add/remove;
3. matched-element update;
4. tensor modify;
5. ordered multiple updates and richer field paths where canonical IR can
   preserve Vespa semantics exactly.

For each operation:

```text
portable schema
-> *_to_ir semantic tests
-> adapter lowering
-> real native execution/persistence
-> public Rust facade
-> Python/TypeScript ergonomic parity
```

Do not approximate unsupported Vespa update semantics.

## 7. Pass 4 — expose already-executable schema/query capability

Prefer facade exposure over protocol widening.

### Schema facade work

Add high-level construction for:

- array-of-struct fields required by `sameElement`;
- position fields required by geo queries;
- predicate fields and their verified configuration subset.

### Query facade work

Close helper gaps for already executable nodes such as:

```text
near / ordered-near
alternatives
userInput / userQuery
nonEmpty
labels
IN / exists conveniences where missing
```

Complete non-default search-field weighting when the native path supports it.

### SDK parity work

- Rust: advanced UpdateBuilder parity, YQL/property conveniences, and missing
  grouping/query helpers with executable semantics.
- Python/TypeScript: fill ergonomic helpers where only generated low-level
  support currently exists.

Every helper must lower to the same canonical IR as its low-level equivalent.

## 8. Pass 5 — cross-language product conformance

Create one shared real-engine corpus for:

```text
Rust direct
Python gRPC/UDS
TypeScript Node gRPC/UDS
TypeScript browser gRPC-Web/TCP
```

Cover at minimum:

```text
create/open
insert/upsert/get/update/delete
text + typed query
textual + structured filter
sort + pagination + projection
facet/group/aggregation
ranking + two-phase
scalar + tensor query inputs
exact + approximate nearest-neighbor/hybrid
summary/docsums
highlighting
close/reopen
portable errors
coverage/degradation
```

Compare semantic outcomes, not language-specific object formatting.

## 9. Pass 6 — packaging, recovery, and measured hardening

After the semantic/result gaps are closed:

- produce clean-machine packages for Rust, `searchkerneld`, Python, and
  TypeScript/Node;
- verify browser gRPC-Web deployment separately from local UDS mode;
- run crash/recovery at public product boundaries;
- run sanitizers and long mutation/search traces;
- measure startup, RSS/PSS, file descriptors, disk growth, indexing throughput,
  query latency, UDS overhead, TCP overhead, and gRPC-Web overhead;
- slim dependencies only when measurements justify it.

## 10. Later / demand-driven work

Add another language SDK only after a real product need exists:

```text
flatc models
+ standard gRPC stub
+ thin facade
+ shared conformance corpus
```

Potential future slices include:

- sparse/mixed tensors;
- broader HNSW configuration and additional distance-metric behavioral fixtures;
- richer schema inheritance/imports;
- ONNX/model assets;
- broader YQL/grouping syntax.

The broad adversarial audit pass remains parked until the current closure
frontier is stable. Grouping continuations remain deliberately deferred.

## 11. Definition of done for new work

A capability is done only when all promised layers are explicit:

```text
represented
semantic
executable
ergonomic where promised
verified against the real engine
covered by the relevant cross-language/release gate
```

A feature is not complete merely because its FlatBuffers type exists, an IR node
exists, or one SDK can construct it.
