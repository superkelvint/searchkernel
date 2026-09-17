# Architecture

Search Kernel is a Rust interface to the local C++ search engine used by Vespa.

The project does not redesign Vespa's search architecture. Its purpose is to make the existing engine usable as a normal embedded Rust library and to expose the same Search Kernel product API to other languages through a small local RPC boundary.

The architecture deliberately separates three concerns:

```text
portable domain contract      FlatBuffers
semantic meaning              Search Kernel IR
execution                     Vespa
```

Transport is plumbing around that model. It must not become another semantic implementation.

---

## Architectural commitments

1. **FlatBuffers is the sole canonical portable domain contract.**
2. **`searchkernel-ir` is the sole portable semantic boundary.** Defaults, validation, normalization, and explicit request meaning are resolved through `*_to_ir()`.
3. **Rust remains directly embedded.** The first-party Rust API does not call gRPC and does not serialize through the portable wire path merely to reach the local engine.
4. **First-party non-Rust clients use gRPC.** The normal local transport is gRPC over a Unix-domain socket on Unix-like systems, with TCP available where appropriate.
5. **Protobuf is transport framing only.** It does not duplicate the Search Kernel domain model.
6. **The only required native FFI boundary is Rust ↔ Vespa C++.** The former portable client C ABI and per-language native transports are not part of the final architecture.
7. **Language SDKs are thin façades.** `flatc` generates FlatBuffers model types; standard gRPC tooling generates RPC stubs; Search Kernel hand-writes only the small idiomatic layer worth owning.
8. **A new portable capability does not have to become ergonomic and executable everywhere in one change.** Representation, semantic support, backend execution, and SDK sugar are separate stages.

---

## Semantic structure

```text
Portable API
    │
    ▼
FlatBuffers contract
    │
    ▼
API → IR conversion
    │  searchkernel-ir / *_to_ir
    ▼
Search Kernel IR
    │  owned backend-independent Rust model
    ▼
Backend interface
    │
    ▼
Vespa lowering
    │  lower_*
    ▼
Vespa model / runtime
```

The Portable API describes what the caller expressed.

The Search Kernel IR describes the fully validated and explicit meaning of that request.

Backend lowering describes how that meaning is represented by the execution engine.

There must be only one implementation of portable semantics.

---

## Runtime structure

### Rust

Rust uses Search Kernel directly in-process:

```text
Rust application
    │
    ▼
idiomatic Search Kernel Rust API
    │
    ▼
Search Kernel IR
    │
    ▼
searchkernel-ir-adapter
    │
    ▼
searchkernel-vespa-engine
    │
    ▼
searchkernel-vespa-bindings
    │
    ▼
Search Kernel native C++ adapter
    │
    ▼
Vespa C++
```

The Rust product API does not go through gRPC, protobuf, JSON, or a portable client C ABI.

### Non-Rust languages

First-party non-Rust clients use the daemon:

```text
Python / Go / Java / Node / PHP / ...
    │
    ▼
small idiomatic language facade
    │
    ▼
flatc-generated FlatBuffers model types
    │
    ▼
RequestEnvelope FlatBuffer bytes
    │
    ▼
generated gRPC client stub
    │
    ▼
Unix-domain socket by default / TCP where appropriate
    │
    ▼
searchkerneld
    │
    ▼
verify canonical FlatBuffer envelope
    │
    ▼
shared request dispatcher
    │
    ▼
searchkernel-ir / *_to_ir
    │
    ▼
Search Kernel IR
```

The language facade may make the API pleasant. It may not redefine Search Kernel semantics.

---

## Canonical contracts

### FlatBuffers

The `.fbs` schemas under `api/` are normative for the portable domain model.

They define requests, responses, values, query expressions, ranking inputs, grouping, errors, and other portable Search Kernel concepts.

Generated BFBS is compiler IR for Search Kernel tooling. BFBS is derived from the pinned FlatBuffers toolchain and is never normative over the `.fbs` source.

### gRPC transport envelope

The RPC schema is intentionally tiny and domain-agnostic. Conceptually:

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

`Frame.payload` contains a canonical FlatBuffers `RequestEnvelope` or `ResponseEnvelope`.

Do not mirror Search Kernel operations into protobuf messages or one RPC method per portable operation unless a concrete transport requirement later justifies it.

Adding a new Search Kernel operation should normally change the FlatBuffers contract without changing the gRPC schema.

### Why protobuf is not a second domain contract

The gRPC layer knows only that it transports bytes.

Type safety for Search Kernel values comes from `flatc`-generated FlatBuffers types. Type-safe RPC plumbing comes from normal gRPC stub generation. The daemon treats the payload as untrusted bytes until the FlatBuffer root and envelope are verified.

The intended boundary is:

```text
typed FlatBuffer construction
        ↓
FlatBuffer bytes
        ↓
process boundary
        ↓
FlatBuffer verification
        ↓
canonical RequestEnvelope
```

---

## Shared dispatch

Transport-specific code must terminate at one canonical dispatch boundary.

Conceptually:

```text
dispatch(request_flatbuffer_bytes)
    -> response_flatbuffer_bytes
```

The dispatcher owns:

```text
FlatBuffer root/envelope verification
protocol/version checks
RequestPayload discrimination
portable request → *_to_ir()
backend invocation
portable response construction
```

The dispatcher does not own gRPC connection handling, Unix-socket setup, HTTP routing, or language-specific API ergonomics.

No transport may bypass `*_to_ir()` with its own request semantics.

---

## Native boundary

The required native boundary exists only to isolate Rust from Vespa's C++ implementation:

```text
Rust types
    │
    ▼
searchkernel-vespa-bindings
    │
    ▼
Search Kernel C++ adapter
    │
    ▼
Vespa types
```

No Vespa C++ type should appear in the public Rust API.

The native interface should use Search Kernel-owned values and opaque engine handles. Conversion to Vespa-specific types happens inside the C++ adapter.

This is a compatibility boundary. Vespa's internal C++ APIs may change without requiring equivalent changes to Search Kernel's public Rust or RPC APIs.

### Removed native boundary

Search Kernel no longer defines a second portable client C ABI for Python, Go, Java, Node, or other language SDKs.

The former architecture:

```text
language facade
    ↓
NativeTransport
    ↓
portable client C ABI
    ↓
Rust dispatcher
```

is retired.

Per-language cgo/JNI/N-API/PyO3-style transport integrations are not a first-party product requirement.

---

## Rust API

The Rust API is the primary in-process product interface.

It should remain idiomatic while keeping its semantics grounded in the canonical portable model and the capabilities of the underlying engine.

For example:

```rust
let runtime = IndexRuntime::open("./products")?;
let index = runtime.index();

index.upsert("sku-42", &product)?;

let results = index
    .search("wireless headphones")
    .filter(field("in_stock").eq(true))
    .limit(20)
    .execute::<Product>()?;
```

The Rust API may offer ergonomic builders and strongly typed helpers without forcing Rust calls through serialization or RPC.

The wrapper should not invent new search semantics merely to produce a more elegant interface.

---

## Non-Rust client APIs

A non-Rust SDK has two layers.

### Generated low-level layer

Owned by standard tooling:

```text
flatc
    → language FlatBuffers model types

gRPC generator
    → client stub for Execute(Frame)
```

This layer should track the portable contract mechanically.

### Handwritten ergonomic layer

Owned by Search Kernel:

```text
Index
SearchBuilder
FieldRef / filter helpers
result wrappers
language-idiomatic errors
small convenience constructors
```

The ergonomic layer may provide syntax sugar but must not independently define:

```text
defaults
field capability rules
filter legality
ranking semantics
request normalization
portable error meaning
```

Those belong in `searchkernel-ir`.

A newly represented FlatBuffers feature may be available through the low-level generated API before a high-level convenience method is added.

---

## Client generation

`searchkernel-bindgen` is not a universal multi-language SDK compiler.

Standard tools should own standard work:

```text
flatc             language FlatBuffers model generation
gRPC toolchain    RPC client/server stub generation
```

Search Kernel bindgen should exist only where Search Kernel-specific BFBS understanding is valuable, such as:

```text
BFBS normalization and invariants
operation/schema inventory
contract metadata
documentation metadata
conformance fixture support
optional JSON/schema metadata
```

Do not add a new custom language emitter merely because a new client language is introduced.

---

## JSON and browser access

JSON is not the primary cross-language client transport.

A canonical JSON representation may remain useful for:

```text
debugging
fixtures
CLI/manual inspection
admin/dashboard APIs
browser-facing gateways
JSON Schema/OpenAPI-style documentation
```

If retained, JSON must still converge on the canonical FlatBuffers model before semantic processing:

```text
JSON
    ↓
canonical JSON ↔ FlatBuffers codec
    ↓
FlatBuffers
    ↓
*_to_ir()
```

Never implement an independent `JSON → IR` semantic path.

Browser transport is a separate edge concern. The dashboard may use a small HTTP/JSON gateway, gRPC-Web/Connect-style bridge, or another browser-compatible mechanism without changing the portable domain semantics.

---

## Capability stages

To keep contract expansion manageable, support is tracked in four independent stages:

```text
REPRESENTED
    FlatBuffers can express the capability losslessly

SEMANTIC
    *_to_ir() validates and gives it explicit meaning

EXECUTABLE
    the Vespa adapter/backend implements that meaning

ERGONOMIC
    one or more high-level SDK facades provide convenient syntax
```

A feature may intentionally be represented and semantic before it is executable or ergonomic.

This prevents a FlatBuffers expansion from forcing simultaneous edits to every SDK and backend layer.

---

## Responsibilities

Search Kernel owns:

- the canonical FlatBuffers portable contract;
- Search Kernel IR and portable semantics;
- the idiomatic Rust product API;
- the shared FlatBuffer dispatcher;
- the small gRPC transport service;
- thin first-party non-Rust facades;
- safe ownership and lifetime management around the native engine;
- translation between Rust and native values;
- error translation;
- native engine startup and shutdown;
- build and linking integration;
- packaging;
- tests proving the wrapper preserves Vespa behavior.

Vespa owns the search-engine implementation, including unless Search Kernel explicitly chooses otherwise:

- document storage;
- transaction logging and recovery;
- memory and disk indexes;
- index fusion;
- attributes;
- lexical matching;
- query execution;
- ranking;
- BM25;
- ranking features;
- tensors;
- vector search;
- concurrency;
- low-level resource management.

Search Kernel should not duplicate these mechanisms.

---

## Engine version

Search Kernel builds against a pinned Vespa source revision.

An upstream Vespa release does not automatically change Search Kernel.

Updating the embedded Vespa revision is an explicit compatibility exercise:

```text
current Vespa revision
        │
        ▼
candidate revision
        │
        ▼
build native adapter
        │
        ▼
run compatibility tests
        │
        ▼
adopt or reject
```

Most source-level breakage caused by a Vespa upgrade should be isolated to the C++ adapter.

Behavioral and persistent-format compatibility must be verified separately.

---

## Development model

Normal Rust development should not require rebuilding Vespa.

The intended workflow remains:

```text
Vespa build environment
        │
        ▼
native SDK / libraries
        │
        ▼
ordinary Rust workspace
```

A developer working on the Rust API, IR, RPC layer, or language clients should normally be able to run the relevant tests without entering the Vespa build container.

Changes to the native C++ adapter may require rebuilding the native SDK.

The transport migration should be additive until cutover:

```text
build new gRPC path beside old client transport
        ↓
prove canonical equivalence
        ↓
switch first-party clients
        ↓
delete portable client FFI/native transport machinery
```

Do not perform a repository-wide in-place transport refactor.

---

## Scope

Search Kernel remains a local/embedded search product.

It does not define its own:

- distributed architecture;
- replication model;
- storage engine;
- postings format;
- transaction model;
- distributed query execution;
- ranking language;
- result-consistency model.

Rust applications may embed the engine directly.

Other languages normally run the same engine through a local `searchkerneld` process, preferably over a Unix-domain socket where supported. This process boundary is a language interoperability mechanism, not a distributed-search architecture.

If the underlying Vespa engine exposes a capability, Search Kernel may expose it.

If Vespa does not expose a capability cleanly, that is first treated as an integration problem rather than an invitation to design a replacement subsystem.

---

## Guiding rules

When choosing between:

```text
represent Vespa faithfully
```

and:

```text
invent a more general Search Kernel abstraction
```

prefer the first until real usage demonstrates why the second is necessary.

When choosing between:

```text
use standard FlatBuffers/gRPC generation
```

and:

```text
build another Search Kernel-specific transport/client generator
```

prefer the standard tooling.

When adding a portable feature, define its meaning once:

```text
FlatBuffers
    ↓
*_to_ir()
```

and derive transport/client plumbing mechanically around that boundary.
