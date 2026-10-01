# Implementation Plan

This plan starts from the repository as it exists now. Completed milestone
specifications are intentionally not repeated here. Historical contracts and
acceptance details remain under [`reference/vespa/`](../reference/vespa/).

The purpose of this document is to control what is allowed to move next.

## 1. Non-negotiable rules

- Keep one portable semantic path: FlatBuffers -> `searchkernel-ir` -> backend.
- Rust stays direct/in-process; non-Rust clients use the shared dispatcher over
  gRPC/gRPC-Web or the explicitly sanctioned standard-FlatBuffers-JSON HTTP edge.
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
| Portable semantics | canonical FlatBuffers, dispatcher, shared search-resolution architecture |
| Transport | `searchkerneld` UDS/TCP + gRPC-Web |
| Python | generated models + gRPC + managed local facade + E2E |
| TypeScript | generated models + browser gRPC-Web + managed Node facade + E2E |
| PHP | standard FlatBuffers JSON HTTP facade + package + real E2E |
| Ruby | standard FlatBuffers JSON HTTP facade + managed local lifecycle + packaged gem + real E2E |
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

Status: **BLOCKED** after adversarial audit of the schema-fidelity implementation
commit.

Required invariant:

```text
IndexDefinitionIr
    -> public Rust representation / describe
    -> IndexDefinitionIr
== original semantic state
```

The current hidden canonical-IR sidecar does not satisfy this requirement by
itself. The public Rust representation must carry the state faithfully enough
that tests, mutation, reuse, and introspection do not depend on returning an
opaque saved IR clone.

Requirements:

- correct the schema-fidelity regressions so they directly test public Rust
  state and cannot pass through a hidden-IR bypass;
- complete Phases 2–5 of the detailed plan rather than reclassifying Phase 1 as
  implementation completion;
- `definition_to_ir()` and `definition_from_ir()` preserve the promised
  canonical schema state through the public representation;
- `Index::describe()` exposes truthful advanced field/rank/schema state;
- cloning/reusing described fields/profiles remains faithful;
- modifying a rich described definition changes only the requested property;
- absent/present optional state, especially rank phases, remains meaningful;
- Rust create's persisted portable/canonical definitions are consistent with
  the final schema ownership model;
- unsupported Vespa execution remains explicit `Unsupported` and is not
  confused with schema representation fidelity.

### 0B. Canonical semantic resolution and typed queries

Status: **BLOCKED** after adversarial audit.

The normal full SearchRequest path already converges through
`SearchInput -> resolve_search()`. Do not replace that working structure.

Required remediation:

#### Schema/ranking ingress

- introduce one shared unresolved schema-input representation for Rust and
  FlatBuffers ingress;
- resolve field capability defaults once;
- resolve rank-profile/rank-phase presence, normalization and validation once;
- prove full canonical equality for equivalent Rust/portable two-phase profiles,
  including absent/empty/invalid phase cases;
- prove field-default parity for text, keyword, numeric, bool, Position, Tensor,
  HNSW and supported multi-valued shapes;
- do not serialize Rust through FlatBuffers merely to share semantics.

#### SearchRequest bypasses

- make `search_to_ir() -> decode_search_input() -> resolve_search()` the sole
  production semantic route for a portable SearchRequest;
- remove, restrict or delegate legacy component-level query/ranking/sort
  resolvers that apply their own defaults/normalization/error meaning;
- add regressions for the known empty-ranking, typed Contains/Term and invalid-
  sort divergences;
- add a structural guard against reintroducing a second portable semantic path.

#### Preserve the good shared path

- typed-query annotations/defaults continue through
  `normalize_typed_query()` and `validate_typed_query()`;
- grouping, filters, search fields, sorting and presentation controls continue
  to converge at the shared resolver;
- focused typed-query tests and two-phase real-engine tests remain green;
- structural IR-boundary checks are green.

Do not widen the typed-query or ranking contract while fixing ownership.

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

### 0D. Highlighting correctness and closure

The first supported slice remains Vespa-generated dynamic teaser output. Before
closure, resolve the highlighting correctness dispatches:

- `highlighting-teaser-decoder-correctness`;
- `highlighting-field-shape-boundary`;
- `highlighting-query-shape-truthfulness`.

Acceptance must prove:

- real Vespa-generated fragment text;
- no synthetic continuation control bytes in public text;
- literal source markup is preserved rather than interpreted as highlight
  framing;
- disjoint teaser regions have correct fragment boundaries;
- byte spans remain valid on non-ASCII text;
- only field shapes actually executable by the native path are accepted;
- query forms unsupported by pinned Juniper highlighting fail explicitly rather
  than silently omitting spans;
- match spans still cover linguistically analyzed/stemmed matches;
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
