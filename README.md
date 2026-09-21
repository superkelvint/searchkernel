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
For cross-layer feature availability, use the [capability ledger](docs/CAPABILITIES.md).

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
executable for generated language bindings and BFBS checks. Cargo builds both
from the pinned vendored source in a native-sensitive target directory and
checks that the produced parser library can compile and link before accepting
it.

The normal host target directory is `target/host`. The supported project
container uses `target/devcontainer`, and CI uses `target/ci`; these directories
must not be shared because they contain compiler- and libc-specific native
artifacts. The Vespa SDK is likewise published below each environment's target
directory (`native-sdk/linux-x86_64`), rather than in one repository-global
directory. `CARGO_TARGET_DIR` remains available for explicit isolated builds;
`SK_NATIVE_SDK` can override the SDK location when building a deliberately
separate native configuration.

`FLATC_PATH` is still accepted by the standalone schema-generation scripts,
but the Cargo protocol build always uses the matching vendored FlatBuffers
toolchain so headers, `libflatbuffers.a`, and `flatc` cannot be mixed from
different installations.

The native-build regression verifiers are
`bash tools/test-native-environment-fingerprint.sh` and
`bash tools/test-native-build-hardening.sh`.
It uses a temporary target directory to verify warm reuse, flag-driven targeted
invalidation, preservation of an unrelated Rust artifact, and one-shot recovery
after a deliberately corrupted FlatBuffers archive. The ABI workflow does not
restore a native target cache; if a CI cache is added, its key must include the
runner/image, Rust toolchain, target, C/C++ tool identities, libc, FlatBuffers
source checksum, and native configuration.

### Development command surface

Repository checks and generators are available through one root command:

```text
sudo ./dev install flatbuffers
./dev check
./dev check-fast
./dev test ir
./dev test rust-api
./dev test python
./dev regen protocol
./dev verify m13
./dev verify integration
./dev hooks install
./dev worktree create my-task
./dev doctor
```

`core/native/vespa/build-sdk.sh` builds the Vespa adapter SDK with the
environment-specific target directory and records the compiler, target, libc,
C++ runtime, Vespa source/configuration, native source, and relevant native
flags that produced it. Native Rust bindings compare that record and link a
small ABI probe before accepting the SDK. Rebuild the SDK in the selected
environment automatically when the record does not match; if the pinned Vespa
build inputs are unavailable, Cargo reports that prerequisite failure instead
of allowing a mysterious downstream linker failure.

`./dev install flatbuffers` builds the pinned FlatBuffers C++ release with the
position-independent static library and the `lib` layout used by standalone
schema tooling and doctor checks. The Cargo protocol build uses its own pinned
vendored copy and does not consume this installation. The default install
prefix is `/usr/local`. For a non-root installation, set
`FLATBUFFERS_INSTALL_PREFIX`; the command prints the `FLATBUFFERS_CPP_ROOT`
and `FLATC_PATH` exports needed by those standalone tools.

Run `./dev hooks install` once per checkout to enable the repository-managed
pre-commit hook. When `docs/capabilities.json` is staged, it regenerates and
stages `docs/CAPABILITIES.md` and `LIMITATIONS.md` in the same commit.

`./dev doctor` is the first command to run when setup is uncertain. It reports
the Cargo, FlatBuffers, generated protocol, native SDK/ABI, pinned Vespa,
Python, and `searchkerneld` prerequisites individually. Run `./dev --help` for
the complete command list.

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
