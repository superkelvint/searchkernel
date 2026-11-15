# Search Kernel

Search Kernel makes the search engine inside [Vespa](https://vespa.ai/) usable as a local component.

Vespa has a capable C++ engine for indexing, matching, ranking, and storing documents. It also has a lot of infrastructure built around that engine, for good reasons: Vespa was designed to run distributed search applications. But not every application needs a distributed search system. Sometimes you want to create an index, put some documents in it, search them, and keep the index on disk.

That's what Search Kernel is for.

Rust applications can use the engine directly in-process. Python can manage a local daemon, and Python, TypeScript/Node.js, and Ruby can talk to an existing daemon. Search Kernel handles the integration with Vespa; Vespa still does the actual searching.

This is a **0.1.x beta**, currently qualified only on **Linux x86-64 (glibc)**. Installation still depends on release artifacts that may require private access. See [release support](docs/release-support.md) and [installation](docs/getting-started.md) before trying it.

## A small example

Here's the Python managed-local API. It creates an index, inserts three documents, searches with a filter and sort, and reopens the same index. The example is shortened from the [tested release quickstart](docs/release-quickstart.md#python-managed-local). It assumes you've installed the matching beta artifacts and are starting with a fresh `./search-python` directory.

~~~python
from pathlib import Path
from searchkernel import Index, Integer, Keyword, Text

path = Path("./search-python")
documents = [
    {"_id": "quickstart-a", "title": "quickstart alpha", "kind": "guide", "score": 2},
    {"_id": "quickstart-b", "title": "quickstart beta", "kind": "guide", "score": 1},
    {"_id": "quickstart-c", "title": "quickstart other", "kind": "other", "score": 0},
]

with Index.create(
    path,
    fields={
        "title": Text(stored=True, searchable=True),
        "kind": Keyword(stored=True, filterable=True),
        "score": Integer(stored=True, sortable=True),
    },
) as index:
    for document in documents:
        index.insert(document)
    hits = (
        index.search("quickstart")
        .filter("kind = 'guide'")
        .sort("score", "asc")
        .execute()
        .hits
    )
    assert [hit.id for hit in hits] == ["quickstart-b", "quickstart-a"]

with Index.open(path) as index:
    assert index.get("quickstart-b")["score"] == 1

print("Search and reopen: PASS")
~~~

Expected output:

~~~text
Search and reopen: PASS
~~~

The full release examples also repeat the search after reopening. There are [Rust, Python, TypeScript, and Ruby examples](docs/release-quickstart.md), each exercising the same basic operations in its supported deployment mode.

## Why use this?

If all you need is basic full-text search over a SQLite database, start with [FTS5](https://www.sqlite.org/fts5.html). If you're building a Rust application and want to work directly with a Rust search index, [Tantivy](https://github.com/quickwit-oss/tantivy) is worth a look.

Search Kernel is for the case in between: you want a local search component, but you also want access to Vespa's query and ranking machinery. That includes full-text retrieval, filters, sorting, facets and grouping, and supported advanced query paths such as nearest-neighbor and geo search. The [capability ledger](docs/CAPABILITIES.md) describes what actually reaches the engine; the [limitations](LIMITATIONS.md) spell out what doesn't.

The tradeoff is that this isn't a tiny search library. You're running Vespa's native engine, with its native runtime requirements. I haven't included speed or memory comparisons here because they need real, reproducible benchmarks.

If you need replication, clustering, or distributed query execution, use [Vespa](https://vespa.ai/) itself. Search Kernel deliberately doesn't provide those things.

## How it works

The most important decision was **not to write another search engine**.

Search Kernel wraps Vespa's existing C++ engine and replaces only the pieces needed to drive it outside the usual Vespa deployment. Rust calls the engine in-process; other languages go through `searchkerneld`. Both paths eventually reach the same search semantics and native engine.

~~~mermaid
flowchart TD
    Rust["Rust application"] --> IR["Search Kernel IR"]
    GRPC["Python / TypeScript via gRPC"] --> Daemon["searchkerneld"]
    HTTP["Ruby via HTTP"] --> Daemon
    Daemon --> IR
    IR --> Adapter["Rust / C++ adapter"]
    Adapter --> Vespa["Vespa C++ engine"]
~~~

The diagram leaves out protocol details. Non-Rust clients use a [FlatBuffers](api/README.md) contract: Python and TypeScript carry binary envelopes over gRPC; Ruby uses schema-derived FlatBuffers JSON over HTTP. Both reach the canonical dispatcher. The Rust API doesn't make a pointless RPC call just to talk to an engine in the same process.

The shared [intermediate representation](core/crates/searchkernel-ir/README.md) matters. Without it, every client and transport is tempted to make its own decisions about defaults, filters, validation, and errors. Eventually the same query means slightly different things in different languages. Search Kernel resolves those decisions in one place.

For a fuller explanation, see the [architecture](docs/architecture.md) and the [codebase walkthrough](docs/reading-the-codebase.md).

## Some of the less obvious work

**Getting the engine to behave like a library.** Vespa's native code wasn't originally packaged as an ordinary Rust dependency. Search Kernel has to own the boundary between Rust and C++: initialization, cleanup, errors, persistent state, and the native resources the engine keeps alive. The [native adapter](core/native/vespa/README.md) and [engine wrapper](core/crates/searchkernel-vespa-engine/README.md) are good starting points.

**Keeping the API consistent.** The Python, TypeScript, Ruby, and Rust interfaces don't have to look identical. They do have to agree about what an operation means. That's why the [portable contract](api/README.md), [semantic IR](core/crates/searchkernel-ir/README.md), and [client conformance evidence](docs/CLIENT_CAPABILITIES.md) are separate from the language-specific convenience APIs.

**Finding out when we're wrong.** A wrapper can appear to work while returning the wrong result, silently dropping an unsupported option, or never calling the native code it's supposed to test. Search Kernel uses frozen behavior contracts, direct Vespa oracles where possible, and real-engine tests to catch that sort of problem. See the [verification harness](reference/vespa/) and [real-engine query tests](core/crates/searchkernel-ir-adapter/src/tests/query_real_engine.rs). Not every part of the beta has finished qualification; the tests are evidence, not a blanket promise.

## Current state

Search Kernel is under active development. These are the currently qualified **0.1.x beta** modes:

| Mode | Status |
| --- | --- |
| Rust in-process | Supported on Linux x86-64 (glibc) |
| `searchkerneld` | Supported on Linux x86-64 (glibc) |
| Python | Managed-local or connect-to-daemon |
| TypeScript / Node.js | Connect-to-daemon |
| Ruby | Connect-to-daemon (HTTP) |

PHP, browser gRPC-Web, Ruby managed-local distribution, Go, macOS, Windows, and Linux ARM64 are not part of the qualified release, even where some code exists. The public APIs can still change. Advanced Vespa functionality isn't automatically supported merely because the underlying engine has it.

For actual installation requirements, runtime ownership, and the current platform matrix, use the [installation guide](docs/getting-started.md), [release support contract](docs/release-support.md), and [known limitations](LIMITATIONS.md). In particular, public source access is **not** currently a guarantee that the native runtime can be downloaded without credentials.

## Building and contributing

If you're working on Search Kernel itself, start with [BUILDING.md](BUILDING.md) and `./dev doctor`. That guide covers the pinned Vespa SDK, FlatBuffers tools, build environments, and test commands. The [reading guide](docs/reading-the-codebase.md) shows where the important code lives.

Implementation work sometimes uses coding agents. The [contribution workflow](docs/agent-workflow.md) records how changes are scoped, reviewed, and verified. Agent-generated code isn't accepted just because the agent says it works.

The [roadmap](docs/roadmap.md) tracks intended work. For what exists today, use [implementation status](docs/status.md) and the [capability ledger](docs/CAPABILITIES.md).

## License

Search Kernel is licensed under [Apache 2.0](LICENSE), the same license as Vespa. The indexing, matching, storage, and ranking engine belongs to Vespa; this project builds the local integration and application APIs around it.
