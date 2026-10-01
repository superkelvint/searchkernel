# How to read Search Kernel

This document is a guided tour for people who understand software systems but
do not read Rust every day. It explains what the repository does, where each
responsibility lives, and the order in which to read the code.

The short version is:

```text
Rust application
  -> public Rust API
  -> Rust resolver input
  -> validated owned Search Kernel IR
  -> Vespa backend adapter
  -> Vespa C++ engine

Non-Rust application
  -> FlatBuffers/gRPC transport
  -> searchkernel-dispatch
  -> validated owned Search Kernel IR
  -> Vespa backend adapter
  -> Vespa C++ engine
```

Search Kernel is a wrapper. The search engine is Vespa. Search Kernel owns the
interfaces, conversions, lifetimes, errors, packaging, and compatibility work
needed to make that engine usable as a library.

## Start here

Read these files in this order:

1. [README.md](../README.md) — what the project is for.
2. [docs/status.md](status.md) — what is present in the current tree.
3. [docs/CAPABILITIES.md](CAPABILITIES.md) — current cross-layer feature status.
4. [docs/CLIENT_CAPABILITIES.md](CLIENT_CAPABILITIES.md) — generated per-client/runtime evidence matrix.
5. [docs/architecture.md](architecture.md) — the architectural boundaries.
6. [docs/roadmap.md](roadmap.md) — intended sequencing, not live status.
7. [api/README.md](../api/README.md) — the language-neutral data contract.
8. [core/crates/README.md](../core/crates/README.md) — the Rust crate map.
9. [core/crates/searchkernel/README.md](../core/crates/searchkernel/README.md) — the API
   an application uses.
10. [core/crates/searchkernel-ir/README.md](../core/crates/searchkernel-ir/README.md)
   — the semantic center.
11. [core/crates/searchkernel-vespa-engine/README.md](../core/crates/searchkernel-vespa-engine/README.md)
   — the Vespa-specific adapter.
12. [core/native/vespa/README.md](../core/native/vespa/README.md) — the C++ boundary.

Then trace one complete operation. Search is the best first example because it
passes through every important layer.

## The repository map

```text
api/
  FlatBuffers schemas shared by all languages

core/crates/searchkernel/
  Product-facing Rust API, fluent builders, and direct Rust-to-IR conversion

core/crates/searchkernel-ir/
  Validation, defaults, normalization, owned IR, and portable results

core/crates/searchkernel-protocol/
  Generated Rust views of the FlatBuffers schemas

core/crates/searchkernel-dispatch/
  Canonical envelope validation, routing, and response construction

core/crates/searchkernel-protocol/src/json.rs
  Generic standard FlatBuffers JSON ↔ BFBS boundary

core/crates/searchkernel-testkit/
  Recording backends, scripted backends, and semantic conformance tools

core/crates/searchkernel-vespa-engine/
  Safe Rust wrapper for the native adapter, linguistics, and config generation

core/crates/searchkernel-vespa-bindings/
  Raw Rust declarations for the native ABI


core/native/vespa/
  Search Kernel-owned C++ adapter around Vespa

target/<environment>/native-sdk/
  Environment-specific generated/prebuilt native SDK consumed by the Rust
  bindings; see [native-build-provenance.md](native-build-provenance.md)

clients/
  Non-Rust language client package roots

searchkerneld/
  Daemon and HTTP transport host

searchkernel-web/
  Administration dashboard

upstream/vespa/
  Pinned, read-only Vespa source used as behavioral authority

reference/
  Frozen contracts, scenarios, goldens, oracles, and verifiers
```

Generated files and the pinned upstream checkout are reference material, not
the first place to look for wrapper design. For the direct Rust path, start at
`core/crates/searchkernel/src/index/mod.rs`; for the portable path, start at
`core/crates/searchkernel-dispatch/src/lib.rs`.

## The most important distinction: three representations

The same user intent can appear in three different forms. The direct Rust path
does not need to construct the portable form; both paths converge at owned IR.

### 1. Public Rust values

These are the types an application sees:

```rust
let result = index
    .search("headphones")
    .filter(field("price").lte(500.0))
    .limit(20)
    .execute::<Product>()?;
```

These values are ergonomic and intentionally hide the wire format. The public
API should not know about FlatBuffers, Vespa C++ classes, raw pointers, or
backend-specific details.

Read:

- `core/crates/searchkernel/src/lib.rs`
- `core/crates/searchkernel/src/index/mod.rs`
- `core/crates/searchkernel/src/search.rs`
- `core/crates/searchkernel/src/filter.rs`
- `core/crates/searchkernel/src/schema.rs`
- `core/crates/searchkernel/src/patch.rs`

### 2. Portable FlatBuffers values

`core/crates/searchkernel/src/portable_definition.rs` contains the one narrow
encoder retained for manifest provenance. Runtime Rust requests go directly to
the semantic resolvers in `core/crates/searchkernel-ir`; they do not serialize
through FlatBuffers or gRPC merely to reach the local engine.

FlatBuffers values are messages stored in bytes. The generated Rust types are
borrowed views into those bytes. They are useful at the boundary, but they are
not the long-lived internal model.

Read:

- `api/searchkernel.fbs`
- `core/crates/searchkernel/src/portable_definition.rs`
- `core/crates/searchkernel-protocol/src/generated/`

### 3. Owned Search Kernel IR

`core/crates/searchkernel-ir` validates portable FlatBuffers views and resolves
direct Rust inputs into owned values. An IR value has no dependency on the
original byte buffer and no dependency on Vespa.

For example, a search request becomes an owned `SearchRequestIr` containing:

- a validated query;
- resolved search fields;
- a normalized filter;
- validated sorting and facets;
- explicit ranking, limit, offset, projection, and highlighting settings.

Read:

- `core/crates/searchkernel-ir/src/model.rs`
- `core/crates/searchkernel-ir/src/api_to_ir/mod.rs`
- `core/crates/searchkernel-ir/src/api_to_ir/resolve/mod.rs`
- `core/crates/searchkernel-ir/src/backend.rs`

This is the key boundary in the whole project:

```text
Rust resolver input or portable wire representation
  -> resolve and validate once
  -> owned, explicit meaning
  -> backend execution
```

## A complete search walkthrough

Use this path when learning the code.

### Step 1: the caller builds a request

The caller starts at `Index::search` in
`core/crates/searchkernel/src/index/mod.rs`. That returns a `SearchBuilder` from
`search.rs`.

Each builder method stores one piece of caller intent:

```text
search text
filters
sorts
facets
ranking
projection
highlighting
language
limit and offset
```

The builder does not validate whether a field exists or whether a field is
sortable. That belongs to the core semantic layer, where the index definition
is available.

### Step 2: public values become resolver input

`SearchBuilder::execute` calls back into `Index::execute_search`.

`execute_search` constructs a `SearchInput` and calls the shared semantic search
resolver directly. It does not build a FlatBuffers message. Portable clients
enter through `searchkernel-dispatch`, decode their FlatBuffers request into the
same unresolved input, and converge at the same `SearchRequestIr`.

The shared resolver is deliberate: it keeps portable semantics in one place
while allowing the first-party Rust API to remain an in-process API.

### Step 3: the request becomes owned IR

The Rust path calls `resolve_search` with a Rust-native `SearchInput`. Portable
clients decode their `api::SearchRequest` into that same input and call
`resolve_search` through `IndexContext::search_to_ir`.

The search resolver coordinates the semantic conversion. It delegates to helpers
for:

```text
query
search fields
filter
sorts
facets
ranking
language
projection
highlighting
```

This is where defaults and validation happen. Examples:

- portable legacy empty text queries retain their existing `MatchAll` behavior;
  the Rust facade rejects blank `search(text)` calls and uses `browse()` for
  the explicit `MatchAll` source;
- omitted limits receive the defined default;
- unknown fields are rejected;
- text queries require searchable fields;
- duplicate fields are rejected where the contract requires uniqueness;
- invalid field/value combinations become portable errors with paths.

The result is an owned `SearchRequestIr`.

### Step 4: the backend executes the IR

The `Backend` trait in `core/crates/searchkernel-ir/src/backend.rs` accepts IR.
The public facade and portable dispatcher both use the semantic resolver, and
the backend does not decide what omitted wire fields mean.

There are several backend implementations for different purposes:

- `RecordingBackend` records IR for facade tests;
- `ScriptedBackend` returns predefined owned results for facade tests;
- the Vespa integration executes through the native engine.

The recording and scripted backends are test tools. They are not substitutes
for Vespa in native acceptance tests.

### Step 5: results come back through an ownership boundary

The backend returns owned core result values. The public crate converts those
values in `core/crates/searchkernel/src/result.rs`:

```text
core Value
  -> JSON value
  -> serde deserialization
  -> caller's Product type
```

This is why `execute::<Product>()` can return a normal Rust struct while the
backend remains independent of that struct.

## A complete mutation walkthrough

Mutation follows the same first three steps.

For `index.upsert("sku-1", &product)`:

```text
Index::upsert
  -> serde serialization
  -> resolve_upsert
  -> UpsertRequestIr
  -> Backend::upsert
```

`insert`, `patch`, `delete`, and `get` use the same pattern. Start in
`core/crates/searchkernel/src/index/mod.rs`, then follow the matching
`resolve_*` function and finally the corresponding `Backend` method.

The important rule is that the public facade never skips the core conversion
path, even when a test backend is being used.

## How the native Vespa path fits

The high-level facade and the native adapter are intentionally separate.

```text
searchkernel
  -> searchkernel-ir resolver
  -> backend implementation
  -> searchkernel-vespa-engine
  -> searchkernel-vespa-bindings
  -> core/native/vespa C++ adapter
  -> Vespa C++
```

`searchkernel-vespa-bindings` contains raw function declarations and opaque native
handle types. `searchkernel-vespa-engine` is the safe Rust wrapper around them.

The C++ adapter in `core/native/vespa` is the only layer that should include Vespa
C++ headers or use Vespa implementation classes. Rust sees Search Kernel-owned
values and opaque handles instead.

When reading a native call, keep asking four questions:

1. Who allocates this value?
2. Who owns it now?
3. How long is each borrowed pointer valid?
4. Who destroys or frees it, including error paths?

The answer should be visible in the nearby Rust guard, C++ cleanup path, or
ABI comment. If it is not visible, that is a boundary readability problem.

## Linguistics and configuration are special cases

Most search behavior belongs to Vespa. Two supporting areas are implemented in
Rust because the embedded setup needs replacements for parts of Vespa's usual
Java request/configuration plane.

### Linguistics

Read `core/crates/searchkernel-vespa-engine/src/linguistics.rs` first, then
`linguistics/stemmer.rs`.

The main concepts are:

- tokenization divides source text into classified spans;
- normalization changes token values but not source offsets;
- stemming reduces terms according to the pinned Vespa behavior;
- document and query analysis are related but not identical operations.

Do not change this code based only on what seems natural for English. Compare
it with the pinned Vespa source and oracle behavior.

### Configuration

Read `core/crates/searchkernel-vespa-engine/src/schema.rs` in this order:

```text
Schema and Field builders
  -> validate_schema
  -> DerivedConfig
  -> render configuration text
  -> write to a temporary directory
  -> install atomically
```

The temporary-directory step prevents partially generated configuration from
being mistaken for a complete configuration.

## How to read the tests

Tests are organized by what they prove.

### Public API tests

`core/crates/searchkernel/tests/api.rs` checks that fluent public calls cross the
core boundary as the expected IR. It commonly uses `RecordingBackend` and
`ScriptedBackend`.

These tests prove facade conversion and result mapping. They do not prove that
Vespa executed a query.

### Core semantic tests

`core/crates/searchkernel-ir/tests/semantics.rs` is the detailed semantic matrix.
It covers defaults, presence, malformed unions, validation, errors, and owned
result encoding.

When changing core conversion, read the relevant semantic test before touching
the implementation. The expected behavior is defined by the contract and
tests, not by whichever implementation currently exists.

### Vespa adapter tests

`core/crates/searchkernel-vespa-engine/src/lib.rs`, `schema.rs`, and `linguistics.rs`
contain focused adapter tests. These check native conversion, configuration
generation, and compatibility behavior.

### Acceptance verifiers

The `reference/` tree contains stronger checks than ordinary unit tests. A
verifier may check:

- exact public API shape;
- exact golden output;
- native linkage and symbols;
- boundary structure;
- cumulative milestone behavior.

Never change a golden or weaken a verifier simply because an implementation is
inconvenient.

## Rust concepts you need while reading

You do not need to become a Rust expert, but these translations help.

| Rust notation | Meaning here |
| --- | --- |
| `Result<T, E>` | An operation returns either a value or an error. `?` returns the error early. |
| `Option<T>` | A value may be absent. |
| `&T` | A borrowed reference; someone else owns the value. |
| `impl Into<String>` | The caller may provide several string-like input types. |
| `T: Serialize` | The caller's type can be converted to JSON. |
| `<'a>` | A lifetime describing how long a borrowed value is valid. |
| `Arc<T>` | Shared ownership of a thread-safe value. |
| `Drop` | Cleanup that runs when an owner goes away. |
| `match` | An explicit switch over enum variants. |
| `?` | Stop this operation and return the error if the previous operation failed. |
| `pub(crate)` | Visible inside this crate, but not to external callers. |
| `unsafe` | The compiler cannot prove the operation is safe; the surrounding code must document and uphold the rules. |

The most important non-Rust idea is ownership. Whenever a value crosses a
boundary, identify its owner and lifetime before trying to understand the
syntax.

## Safe change workflow

For a feature-propagation checklist, use the [change recipes](change-recipes.md).
They complement this guide with ordered paths for portable schema, IR, Vespa,
native ABI, result, ranking, grouping, update, and SDK changes.

When modifying a capability:

1. Find the public entry point.
2. If it is a Rust call, follow its resolver-input conversion; if it is a
   portable call, follow the FlatBuffers encoder and dispatcher.
3. Follow the matching resolver and API-to-IR conversion.
4. Find the core semantic test or frozen contract.
5. Follow the backend method.
6. If native, continue through `searchkernel-vespa-engine` and `core/native/vespa`.
7. Change the smallest layer that owns the behavior.
8. Run focused tests first, then the workspace checks.

Before declaring a change complete, confirm:

- public Rust code contains no Vespa types;
- raw unsafe code remains at a dedicated boundary;
- errors are returned rather than hidden or unwrapped;
- ownership is explicit on success and failure paths;
- generated, upstream, and frozen acceptance files were not edited without
  explicit authorization.

## One-sentence mental model

Search Kernel takes friendly Rust input directly, or a portable FlatBuffers
message through the dispatcher, resolves both into explicit owned meaning,
hands that meaning to Vespa, and carefully converts the owned result back into
ordinary application data.
