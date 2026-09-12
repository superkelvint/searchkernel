# Architecture

Search Kernel is a Rust interface to the local C++ search engine used by Vespa.

The project does not attempt to redesign Vespa's search architecture. Its purpose is to make the existing engine usable as a normal native library from Rust and, later, Python.

## Structure

```text
Portable API
    │
    ▼
FlatBuffers contract
    │
    ▼
API → IR conversion
    │  searchkernel-core / *_to_ir
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

The portable API describes what the caller expressed. The Search Kernel IR
describes the fully validated and explicit meaning of that request. Backend
lowering describes how that meaning is represented by the execution engine.

The Portable API and FlatBuffers contract are the language-neutral boundary.
`searchkernel-core` performs API → IR conversion through `*_to_ir` functions.
The Search Kernel IR is an owned, backend-independent Rust model. The backend
interface receives that IR, and Vespa lowering translates it through `lower_*`
functions into the Vespa model and runtime.

The Rust API is the public boundary.

The Vespa C++ implementation is private.

## Responsibilities

Search Kernel owns:

- the Rust API;
- safe ownership and lifetime management;
- translation between Rust and native values;
- error translation;
- native engine startup and shutdown;
- build and linking integration;
- packaging;
- Python bindings;
- tests that verify the wrapper preserves Vespa behavior.

Vespa owns the search-engine implementation.

This includes, unless Search Kernel explicitly chooses otherwise in the future:

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

## Native boundary

The native boundary exists to isolate Rust from Vespa's C++ implementation.

Conceptually:

```text
Rust types
    │
    ▼
Search Kernel native API
    │
    ▼
C++ adapter
    │
    ▼
Vespa types
```

No Vespa C++ type should appear in the public Rust API.

The native interface should use Search Kernel-owned values and opaque engine handles. Conversion to Vespa-specific types happens inside the C++ adapter.

This is primarily a compatibility boundary.

Vespa's internal C++ APIs are allowed to change without requiring equivalent changes to Search Kernel's public Rust API.

## Rust API

The Rust API should remain close to the capabilities of the underlying engine.

Initially, prefer direct representations of real Vespa operations over new Search Kernel abstractions.

For example:

```rust
let engine = Engine::open(path, config)?;

engine.put(document)?;
engine.update(update)?;
engine.remove(id)?;

let result = engine.search(request)?;
```

`Document`, `SearchRequest`, `SearchResult`, schema configuration, and similar types should be idiomatic Rust values, but their semantics should remain grounded in the corresponding Vespa behavior.

Convenience APIs may be added later.

The wrapper should not invent new search semantics merely to produce a more elegant interface.

## Engine version

Search Kernel builds against a pinned Vespa source revision.

An upstream Vespa release does not automatically change Search Kernel.

Updating the embedded Vespa revision is an explicit project change:

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

## Development model

Normal Rust development should not require rebuilding Vespa.

The intended workflow is:

```text
Vespa build environment
        │
        ▼
native SDK / libraries
        │
        ▼
ordinary Rust workspace
```

A developer working on the Rust interface should normally be able to run:

```bash
cargo test --workspace
```

without starting the Vespa build container.

Changes to the native adapter may require rebuilding the native SDK.

This separation is important both for developer iteration and automated coding agents.

## Python

Python bindings sit above the Rust API.

```text
Python
   │
   ▼
PyO3
   │
   ▼
Rust API
```

Python does not call the C++ layer directly.

A capability exposed to Python should first exist as a normal Rust capability unless there is a strong reason otherwise.

## Scope

Search Kernel is currently a wrapper project.

It does not define its own:

- distributed architecture;
- replication model;
- storage engine;
- postings format;
- query planner;
- transaction model;
- schema migration architecture;
- ranking language;
- result-consistency model.

If the underlying Vespa engine exposes a capability, Search Kernel may expose it.

If Vespa does not expose a capability cleanly, that is first treated as an integration problem rather than an invitation to design a replacement subsystem.

## Guiding rule

When choosing between:

```text
represent Vespa faithfully
```

and:

```text
invent a more general Search Kernel abstraction
```

prefer the first until real usage demonstrates why the second is necessary.
