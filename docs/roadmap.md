# Roadmap

This document describes the current baseline and the work that is still worth
planning. Completed milestone definitions live under
[`reference/vespa/`](../reference/vespa/) and historical context lives in
[`history.md`](history.md).

For architectural invariants, read [`architecture.md`](architecture.md). For
the factual repository inventory, read [`status.md`](status.md).

## Current baseline

The following is already present in the repository and should be treated as
foundation rather than future design work:

- native Vespa capability through the M14 implementation surface;
- Phase 9 additive native/query coverage and Phase 10 public Rust real-engine E2E;
- the integrated public product path and `reference/vespa/i` release gate;
- one canonical FlatBuffers -> Search Kernel IR -> Vespa adapter architecture;
- shared dispatcher used by non-Rust transports;
- direct in-process Rust product API;
- `searchkerneld` over UDS/TCP plus the gRPC-Web listener;
- Python gRPC client, managed local daemon lifecycle, ergonomic facade, and E2E;
- TypeScript browser client plus managed-local Node facade and E2E;
- textual filter parsing/binding with clause-local parameters;
- typed queries, YQL subset, sorting, projection, facets/grouping, ranking,
  two-phase reranking, dense tensors, persistence, and nearest-neighbor search;
- HNSW schema/configuration and approximate nearest-neighbor implementation for
  the M14 slice;
- default summary selection, scalar projected docsums, and the first real
  Vespa-generated highlighting path;
- exact-head Rust workspace unit-test CI, ABI-surface CI, Client Capabilities
  verification, and automatic repository Git-hook configuration for local
  test gating.

Implementation presence is not acceptance. A newly merged capability stays open
until its frozen verifier and required real-engine acceptance evidence pass.

The retired portable client FFI / `NativeTransport` architecture must not
return.

## Current frontier — close the remaining correctness work

Do not start another broad capability family until the current tree is green on
its exact head and the remaining semantic/schema acceptance work has converged.

### 0. Multi-index daemon control plane

`searchkerneld` must become a long-lived multi-index service before client
lifecycle parity or release qualification can close. The current single-index
`--index` startup model is a transitional implementation, not the target server
architecture.

Required closure:

- one daemon-owned registry containing multiple independently isolated indexes;
- explicit logical index targeting in the canonical FlatBuffers request path;
- canonical create/open lifecycle operations shared by gRPC and HTTP/JSON;
- a configured daemon data root that maps safe logical index identifiers to
  persistent directories without exposing arbitrary host filesystem paths;
- linearizable same-name create/open behavior and safe lifecycle races;
- persistence/restart with multiple indexes resident or reopenable;
- client lifecycle surfaces that create/open/select indexes through the daemon
  without another language client or per-index daemon restart.

Keep the existing per-index dispatcher as the semantic execution boundary where
practical and place routing/registry ownership above it. Client-handle `close()`
must not implicitly unload a shared daemon index.

Tracking: #844, with protocol work in #845, daemon registry work in #846, and
PHP lifecycle parity in #841.
### 1. Keep exact-head `main` green

This is the first closure gate, not a follow-up task.

Recent reconciliation fixed:

- the multi-valued highlighting compile regression;
- incomplete public `RankProfile` state;
- stale facet result expectations;
- stale generated capability limitations;
- the rich schema round-trip fixture;
- a real `BackendSession` close race where an operation published
  `active == 0` before releasing its backend owner.

The lifecycle/schema fixes were independently proven by a green Rust unit-test
run on commit `d9d14fd6`.

Any later commit that makes the exact current head red returns to the front of
the queue before feature work continues.

### 2. Canonical semantic resolution

The full SearchRequest path now converges through the shared canonical resolver,
and the filter comparison error-domain drift has been repaired.

The remaining closure cluster is the structural ownership work tracked by:

- #43 — guard against duplicate production SearchRequest semantic entry points;
- #143 — guard against duplicate Rust/FlatBuffers schema semantic entry points;
- #144 — adversarially verify that the remediated tree has one semantic path.

Closure requires:

- equivalent Rust and FlatBuffers intent to resolve through shared semantic
  ownership;
- no production component-level bypass that can produce a different canonical
  shape or error domain;
- structural regression guards that fail if a second semantic path is added;
- existing typed-query/ranking behavior remaining green.

Do not redesign the frozen typed-query model while fixing semantic ownership.

### 3. Schema fidelity

Large parts of the public schema representation are now present: complete field
types/capabilities/configuration, named structs, fieldsets, imported fields,
document summaries, constants, ONNX models, richer rank-profile state, and
lossless IR -> public materialization coverage.

The remaining Phase 3/4 closure work is narrower and is tracked primarily by
#113–#116, #39, and #110:

- remove the remaining hidden canonical-sidecar recovery paths;
- make public `IndexDefinition`, `Field`, and `RankProfile` state
  authoritative for Rust -> IR conversion;
- preserve all nested/top-level schema state and absence/presence semantics;
- keep full canonical equality through describe/clone/mutate/reuse;
- retain explicit backend Unsupported behavior rather than silently dropping
  represented schema state.

The required invariant remains lossless canonical schema preservation through
the actual public Rust representation. Backend executability is separate.

### 4. Phase 9 / additive / YQL / grouping acceptance

Re-establish and record the cumulative acceptance state on the actual current
native environment rather than inferring closure from implementation presence.

Verify:

- additive ABI/query behavior;
- portable summary-hit and exact-ID semantics;
- YQL/additive combination rules;
- grouping interactions;
- specialized Phase 9 checks and the cumulative verifier.

### 5. Highlighting closure

The first real Vespa-generated dynamic-teaser path is implemented, and the
earlier cardinality/compile regression has been repaired. Treat highlighting as
an acceptance-hardening slice rather than as an unimplemented feature.

Closure still requires real-engine evidence that the supported path:

- handles continuation framing without leaking control bytes;
- preserves correct byte spans for non-ASCII text;
- exercises long/truncated teasers;
- has explicit query/field-shape compatibility rather than silent fallback;
- preserves multi-valued cardinality semantics at the IR boundary even where
  the pinned native teaser path remains scalar-only.

Keep these separate unless intentionally scheduled:

- custom document-summary definitions;
- arbitrary Juniper rendering controls;
- grouped highlights;
- richer docsum value shapes.

### 6. M14 HNSW acceptance

The M14 implementation is present, and distance-threshold verification now
checks each query stage rather than treating the control as a single aggregate
signal.

Remaining closure is verifier/evidence hardening against the real engine:

- emitted HNSW tensor attribute configuration;
- exact control on the HNSW-enabled field;
- proof that approximate mode selects HNSW rather than exact fallback;
- explore-additional-hits and distance-threshold controls;
- mutable vector updates;
- close/reopen persistence;
- delete/reopen behavior;
- no duplicate/stale evidence stages;
- no Rust-side ANN fallback or shadow graph.

Open M14 hardening items such as #26, #27, and #29 should close only when their
specific frozen evidence criteria pass.

### Frontier exit criteria

The closure frontier is complete only when:

- the exact current `main` head is green on Rust unit tests, ABI surface, and
  all applicable capability/client checks;
- all relevant regression tests exist and pass;
- Phase 9 and M14 real-engine verifier results are recorded;
- no second production semantic path remains;
- public schema round-trips do not depend on hidden canonical sidecars;
- the capability/limitations ledger matches executable reality;
- blocker-focused adversarial review finds no remaining correctness blocker.

## Next — finish the mechanically reproducible release gate

Several pieces now exist independently: Rust workspace unit-test CI, ABI-surface
checks, Client Capabilities verification, generated-document drift checks, and
automatic local Git-hook configuration.

After the closure frontier is stable, compose these with the native/product
verifiers into one current-tree verification entry point that runs the supported
release surface from a clean checkout.

It should cover at least:

- cumulative native gate through M14;
- Phase 9 specialized gate;
- I-series / Phase 10 product gate;
- SearchReply and recovery gates;
- workspace fmt/clippy/tests;
- generated-artifact drift checks;
- `searchkerneld` transport tests;
- Python unit/package/gRPC/E2E;
- TypeScript generate/typecheck/unit/package/managed-local E2E;
- capability/status documentation consistency.

The goal is one documented command whose success means the current release
baseline is reproducible rather than remembered.

## After that — finish the native result/docsum seam

The request side still exposes more than the native result path can faithfully
return.

Complete in small slices:

- richer/non-scalar docsum value decoding;
- custom document-summary definitions in native schema generation;
- grouped `summary()` aggregation hit payloads;
- `Hit.sort_values` from the real engine;
- summary-feature and rank-feature readback;
- typed tensor-valued feature readback.

Do not invent result data in Rust. Every value must cross the real Vespa
result/docsum path.

## Advanced mutation semantics

The remaining portable update surface includes:

- conditional insert/upsert/update/delete;
- collection add/remove;
- matched-element updates;
- tensor modify operations;
- ordered multiple updates to the same target/field where the contract can
  preserve Vespa ordering faithfully;
- richer field-path forms currently rejected.

Implement backend semantics first, then add ergonomic parity in Rust, Python,
and TypeScript.

## Public schema/query facade gaps

Prefer exposing already-proven backend capability over widening the portable
contract again.

Priority gaps include:

- public array-of-struct definitions needed by `sameElement`;
- public position/geo field definitions;
- public predicate field definitions;
- missing high-level helpers for already-supported query nodes;
- non-default search-field weighting;
- Rust/Python/TypeScript ergonomic parity where generated low-level support
  already exists, while keeping implemented PHP/Ruby facades aligned with the
  same portable semantics.

Facades add syntax only. They do not define semantics.

## Cross-language product conformance

Build one canonical real-engine corpus and run it through:

```text
Rust direct embedded API
Python -> gRPC -> searchkerneld
TypeScript/Node -> gRPC -> searchkerneld
TypeScript/browser -> gRPC-Web -> searchkerneld
PHP -> standard FlatBuffers JSON/HTTP -> searchkerneld
Ruby -> standard FlatBuffers JSON/HTTP -> searchkerneld
```

Require equivalent portable meaning and equivalent real-engine outcomes for the
shared supported surface.

## Packaging and release hardening

Once the semantic/result gaps and shared conformance gate are solid:

- produce clean-machine Rust embedded and `searchkerneld` packages;
- package Python managed-local mode;
- package TypeScript/Node and browser builds;
- verify reproducible FlatBuffers/protobuf generation;
- run crash/recovery and reopen tests through supported public entry points;
- run sanitizers and resource/leak testing;
- measure startup, RSS/PSS, indexing throughput, query latency, UDS overhead,
  TCP overhead, and gRPC-Web overhead;
- slim dependencies only where measurements justify it.

## Additional language SDKs only on demand

PHP and Ruby implementations are already present but remain outside the next
beta release-support promise. Go, Java, and other additional SDKs are not
prerequisites for stabilizing the core product.

A new first-party SDK must select one architecture-sanctioned representation path:

```text
binary: flatc-generated models + standard gRPC/gRPC-Web stub
HTTP-first: standard FlatBuffers JSON + the BFBS-backed HTTP execute edge
+ thin idiomatic facade
+ shared conformance corpus
```

No first-party custom native transport or custom RPC runtime should be added.

## Explicitly deferred or out of scope

Do not accidentally pull these into the active frontier:

- grouping continuations;
- raw grouping parser/normalizer;
- full Vespa container/distributed platform;
- another query/filter/ranking language;
- another SDK/transport semantic implementation;
- broad adversarial audit pass while the current closure frontier is still
  moving.

## Current verification gates

Acceptance authority remains the checked-in contracts and verifiers. Important
top-level commands include:

```bash
./dev verify milestone.m14
./dev verify vespa.grouping.yql
./dev verify product.integration
./dev verify vespa.search-reply
./dev verify vespa.recovery
```

Do not infer a passing milestone from implementation presence alone.
