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
- one canonical FlatBuffers V2 -> Search Kernel IR -> Vespa adapter architecture;
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
  Vespa-generated highlighting path.

Implementation presence is not acceptance. A newly merged capability stays open
until its frozen verifier and required real-engine acceptance evidence pass.

The retired portable client FFI / `NativeTransport` architecture must not
return.

## Current frontier — close the semantic/schema correctness work

Do not start another broad capability family until the following in-flight work
has converged and its acceptance state is explicit.

### 1. Schema fidelity

Finish the work tracked in
[`implementation-plan/schema-fidelity-plan.md`](../implementation-plan/schema-fidelity-plan.md).

The required invariant is lossless canonical schema preservation across Rust
introspection/describe boundaries. Do not treat backend executability as a
reason to discard canonical schema information.

Closure requires:

- the comprehensive schema-fidelity regression corpus green;
- Rust -> IR and IR -> Rust preservation for the promised surface;
- `Index::describe()` semantic round-trip fidelity;
- unsupported backend execution remaining explicit rather than silently lost.

### 2. Canonical search resolution and typed-query boundary

Keep exactly one semantic orchestration path for search intent.

Rust facade ingress and FlatBuffers ingress may decode differently, but both
must converge on the same unresolved representation and canonical resolver
before resolved Search Kernel IR exists.

Close remaining typed-query boundary leaks and structural checks without
redesigning the frozen typed-query model.

### 3. Phase 9 / additive / YQL / grouping acceptance

Re-establish the current cumulative acceptance state rather than assuming the
older audit work is closed.

In particular, verify:

- additive ABI/query behavior;
- portable summary-hit and exact-ID semantics;
- YQL/additive combination rules;
- grouping interactions;
- specialized Phase 9 checks and the cumulative verifier on the actual native
  environment.

### 4. Highlighting closure

The first real Vespa-generated dynamic teaser path is now implemented. Close
that slice with real-engine and client E2E evidence.

Keep these as separate future capabilities unless intentionally scheduled:

- custom document-summary definitions;
- arbitrary Juniper rendering controls;
- grouped highlights;
- richer docsum value shapes.

### 5. M14 HNSW acceptance

The M14 implementation is present. Closure still requires the frozen M14
contract to pass against the real engine, including:

- emitted HNSW tensor attribute configuration;
- exact control on the HNSW-enabled field;
- approximate search;
- explore-additional-hits and distance-threshold controls;
- mutable vector updates;
- close/reopen persistence;
- delete/reopen behavior;
- no Rust-side ANN fallback or shadow graph.

### Frontier exit criteria

The closure frontier is complete only when:

- previously red structural/capability checks are green;
- all relevant unit/regression tests exist and pass;
- Phase 9 and M14 real-engine verifier results are recorded;
- no second search semantic path remains;
- the capability/limitations ledger matches executable reality;
- blocker-focused review finds no remaining correctness blocker.

## Next — make the current release state mechanically reproducible

After the closure frontier is stable, create one current-tree verification entry
point that runs the supported release surface from a clean checkout.

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
  already exists.

Facades add syntax only. They do not define semantics.

## Cross-language product conformance

Build one canonical real-engine corpus and run it through:

```text
Rust direct embedded API
Python -> gRPC -> searchkerneld
TypeScript/Node -> gRPC -> searchkerneld
TypeScript/browser -> gRPC-Web -> searchkerneld
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

Go, PHP, Java, and other SDKs are not prerequisites for stabilizing the core
product.

A new first-party SDK must use:

```text
flatc-generated models
+ standard gRPC stub
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
./reference/vespa/verify-through.sh m14
./reference/vespa/phase9/verify-phase9.sh
./reference/vespa/verify-through.sh i
./reference/vespa/verify-through.sh searchreply
./reference/vespa/recovery/verify-recovery.sh
```

Do not infer a passing milestone from implementation presence alone.
