# Roadmap

This document contains only the current baseline and work that is still worth
planning. Completed milestone definitions have been removed; their contracts,
oracles, and verification history remain under [`reference/vespa/`](../reference/vespa/)
and [`history.md`](history.md).

For architectural invariants, read [`architecture.md`](architecture.md). For a
repository inventory, read [`status.md`](status.md).

## Current baseline

The following is already implemented and should be treated as foundation, not
future roadmap work:

- cumulative native Vespa baseline through M13;
- Phase 9 additive native/query coverage and Phase 10 public Rust real-engine E2E;
- the integrated public product path and `reference/vespa/i` release gate;
- one canonical FlatBuffers V2 -> Search Kernel IR -> Vespa adapter path;
- shared dispatcher used by non-Rust transports;
- direct in-process Rust product API;
- `searchkerneld` over UDS/TCP plus the gRPC-Web listener;
- Python gRPC client, managed local daemon lifecycle, ergonomic facade, and E2E;
- TypeScript browser client plus managed-local Node facade and E2E;
- textual filter parser/binder with clause-local bindings and structured-filter equivalence;
- typed queries, YQL subset, sorting, projection, facets/grouping, ranking, two-phase reranking,
  dense tensors, persistence, nearest-neighbor search, and degradation metadata;
- default summary-class selection and projected scalar docsums through the real engine.

The old portable client FFI / `NativeTransport` architecture is retired and must
not return.

## Current frontier

### 1. Finish the native result/docsum seam

This is the largest remaining execution gap because the request side is ahead of
what the native result ABI can return.

Complete, in roughly this order:

- non-scalar/richer docsum value decoding;
- custom document-summary definitions in native schema generation;
- grouped `summary()` aggregation hit payloads;
- `Hit.sort_values` from the real engine;
- summary-feature and rank-feature readback;
- typed tensor-valued feature readback instead of opaque bytes;
- native highlighting fragments/spans.

Do not invent result data in Rust. Every item above must cross the real Vespa
result/docsum path.

### 2. Finish advanced mutation semantics

The basic mutation path and arithmetic updates are already real-engine features.
The remaining portable update surface is:

- conditional insert/upsert/update/delete;
- collection add/remove;
- matched-element updates;
- tensor modify operations;
- ordered multiple updates to the same target/field where the portable contract
  can represent Vespa's ordering faithfully;
- richer field-path forms that are currently rejected.

After backend support exists, expose the same capability ergonomically in Rust,
Python, and TypeScript rather than adding SDK-local semantics.

### 3. Close the public schema/query facade gaps

The backend can execute more query shapes than the public high-level schema
facades can currently construct. Close that mismatch before widening the
portable contract again.

Priority gaps:

- public array-of-struct definitions needed by `sameElement`;
- public position/geo field definitions;
- public predicate field definitions;
- high-level helpers for already-supported typed query nodes such as proximity,
  alternatives, `userInput`/`userQuery`, `nonEmpty`, labels, and other missing
  SDK-specific constructors;
- non-default search-field weighting;
- Rust parity for advanced update/YQL conveniences and Python/TypeScript parity
  where generated low-level support exists but ergonomic sugar does not.

The rule remains: representation and semantics stay centralized; facades add
syntax only.

### 4. Make cross-language conformance a release gate

Python and TypeScript now have substantial real-engine E2E coverage, but the
repository still needs one explicit shared conformance gate.

Build a single canonical scenario corpus and run it through:

```text
Rust direct embedded API
Python -> gRPC -> searchkerneld
TypeScript/Node -> gRPC -> searchkerneld
TypeScript/browser -> gRPC-Web -> searchkerneld
```

Require equivalent portable meaning and equivalent real-engine outcomes for the
shared capability set. Keep language-specific ergonomic tests in addition to
this corpus.

The capability ledger must include TypeScript and must be generated or checked
against executable evidence so it cannot silently fall behind the code again.

### 5. Packaging and release hardening

Once the result seam and cross-language gate are solid, finish the product-level
release work:

- clean-machine Rust embedded package;
- clean-machine `searchkerneld` package;
- Python package with managed-local daemon support;
- TypeScript/Node package and browser build;
- reproducible FlatBuffers/protobuf generation and drift checks;
- crash/recovery and reopen tests through supported public entry points;
- sanitizer/resource/leak runs;
- startup, RSS/PSS, indexing throughput, query latency, UDS RPC overhead, and
  gRPC-Web/TCP overhead measurements;
- dependency/runtime slimming where measurements justify it.

### 6. Additional language SDKs only on demand

Go, PHP, Java, and other SDKs are not prerequisites for stabilizing the core
product. Add one only when there is a concrete user/product requirement, using
the existing pattern:

```text
flatc-generated models
+ standard gRPC stub
+ thin idiomatic facade
```

No new first-party native transport or custom client runtime should be created.

## Explicitly deferred or out of scope

The following must not accidentally reappear as near-term roadmap work:

- **Grouping continuations:** deliberately deferred. Preserve protocol/result
  compatibility, but do not implement native continuation execution now.
- **Raw grouping parser/normalizer:** optional expert work; structured grouping
  is the supported product path.
- **Full Vespa container platform:** distributed dispatch, replication,
  federation, Config Server/Proxy, MessageBus, Slobrok, search chains, and
  container query profiles remain outside the embedded product.
- **A second query/filter/ranking language:** do not create one.
- **A second semantic path in an SDK or transport:** do not create one.

## Current verification gates

The repository's existing authority remains the checked-in contracts and
verifiers. The important top-level gates are:

```bash
./reference/vespa/verify-through.sh m13
./reference/vespa/phase9/verify-phase9.sh
./reference/vespa/verify-through.sh i
./reference/vespa/verify-through.sh searchreply
./reference/vespa/recovery/verify-recovery.sh
```

The client release gate should additionally run the Python and TypeScript test,
package, generation, and managed-local E2E suites. The roadmap item above is to
make that one explicit, repeatable command rather than relying on human memory.
