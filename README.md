# Search Kernel

Search Kernel is a Rust interface to Vespa's native search engine, packaged so it can be embedded directly in an application instead of run as a separate service.

Vespa's C++ engine does the actual work: storage, indexing, ranking, query execution. Search Kernel does not reimplement any of it. It gives that engine a normal Rust API and a Python gRPC client today; additional language facades are added when there is a concrete product need.

```python
from searchkernel import Index

index = Index.open("./search")

index.put({
    "_id": "1",
    "title": "The quick brown fox",
})

results = index.search("brown fox")

index.update("1", {"title": "The slow brown fox"})
index.delete("1")
```

The same operations in Rust:

```rust
let engine = Engine::open(path, config)?;

engine.put(document)?;
engine.update(update)?;
engine.remove(id)?;

let result = engine.search(request)?;
```

## Why

Running a search cluster to get full-text search, filtering, and ranking is a lot of infrastructure for most applications. Most applications don't need a cluster. They need an index that lives next to their data and behaves like any other embedded store.

Vespa already solves the hard problems here — ranking, tensors, vector search, index fusion, durability — at a level of maturity that would take years to reproduce. Search Kernel doesn't try. It makes that engine usable as a library.

If you are new to the codebase, start with the [codebase reading guide](docs/reading-the-codebase.md).
Agents should check the [current status](docs/status.md) before using the roadmap.

## What it is

- **Embedded.** The engine runs in your process. An index is a directory, not a service.
- **Mutable.** Put, update, and delete are normal operations, not workarounds.
- **Durable.** Indexes survive restarts and recover on reopen.
- **Rust-first.** The Rust API is the primary interface. JavaScript, PHP, Go, and Python are built on top of it, not alongside it — a capability shows up in a binding because it exists in the Rust API first, not the other way around.
- **A thin wrapper.** Whatever Vespa can do, Search Kernel can expose. Whatever Vespa doesn't do, Search Kernel doesn't invent. See [architecture.md](docs/architecture.md) for the reasoning behind that rule and where the boundary sits.

## What it is not

Search Kernel is not a distributed search engine and isn't going to become one. No replication, no clustering, no distributed query execution. If you need those, use Vespa directly, or another distributed search system — that's a better fit than asking a single-node wrapper to grow into one.

It also doesn't have its own ranking language, storage engine, or schema migration model. Those are Vespa's, and Search Kernel exposes them rather than replacing them.

## Installing

Search Kernel links against Vespa's native libraries at build time. None of the bindings are pure-language packages.

*(Installation instructions — supported platforms, native dependencies, build steps, per language — go here.)*

### Development prerequisites

Building and testing the workspace uses the official FlatBuffers C++ release
`25.12.19` for the schema-aware JSON boundary and the matching `flatc`
executable for generated language bindings and BFBS checks. The default
installation locations are `/usr/local/include`, `/usr/local/lib`, and
`/usr/local/bin`.

For a non-standard installation, set `FLATBUFFERS_CPP_ROOT` for Cargo and
`FLATC_PATH` for schema and Python-generation checks. All installed components
must be version `25.12.19`.

FlatBuffers is the sole portable data contract. Non-Rust RPC carries canonical
FlatBuffer bytes over generated gRPC. Standard FlatBuffers JSON is the only
supported JSON representation; it is parsed to FlatBuffers before dispatch and
printed from FlatBuffers for responses. Language SDKs use flatc-generated
models and do not maintain parallel full-schema DTOs or handwritten codecs.

## Language bindings

| Language   | Status |
|------------|--------|
| Rust       | Direct embedded API |
| Python     | gRPC transport and ergonomic facade present |
| JavaScript | Planned/on demand |
| Go         | Planned/on demand |
| PHP        | Planned/on demand |

Implemented bindings expose the same underlying operations. If one binding supports something another doesn't yet, that's a gap to close, not a difference in design.

## Repository layout

```text
api/               normative Portable API schemas
core/              embeddable Rust crates and native SDK
clients/           non-Rust language clients
searchkerneld/     daemon and HTTP transport host
searchkernel-web/  administration dashboard
tools/             generators and build tooling
reference/         frozen contracts and verifiers
```

## Status

See [docs/status.md](docs/status.md) for the current implementation inventory
and verification entry points. The project remains under active development;
the roadmap is intended sequencing, not a status report.

The API will change before it stabilizes.

## License

Apache License 2.0, matching Vespa's license.

## Contributing

*(contributing guidelines, or a pointer to them)*
