# Search Kernel

**Powerful search. Without operating a search cluster.**

Search Kernel brings [Vespa](https://vespa.ai/)'s native C++ search engine to local applications: an **in-process Rust library** and a **local daemon with Python, TypeScript/Node.js, and Ruby clients**. Build full-text search, filtering, sorting, ranking, and persistent indexes without deploying Vespa's distributed services or Java application stack.

The engine is Vespa's; Search Kernel does not reimplement storage, indexing, matching, or ranking. It supplies the application-facing APIs, native lifecycle, configuration, packaging, and shared semantics needed to use that engine as a local component.

**0.1.x experimental beta · Linux x86-64 (glibc) only.** See [supported release modes](docs/release-support.md) and [known limitations](LIMITATIONS.md) before adopting it. Release artifacts and some native-runtime downloads may require access to private release infrastructure today; making this source repository public alone would not make every installation path publicly usable.

[Quickstart](#try-it-in-python) · [How it works](#how-it-works) · [Installation](docs/getting-started.md) · [Architecture](docs/architecture.md) · [Release support](docs/release-support.md)

## What it gives you

- **Full-text search and relevance ranking.** Use Vespa's native text indexing, matching, and supported ranking capabilities instead of building a separate search implementation.
- **Structured queries on application data.** Combine text retrieval with typed filters, sorting, facets, and grouping where supported.
- **Mutable, persistent indexes.** Insert, update, delete, close, and reopen search data rather than rebuilding a static index for every change.
- **A local deployment model.** Embed directly in Rust; use Python's managed-local daemon mode, or connect Python, TypeScript/Node.js, and Ruby to a separately run daemon.
- **Room to grow into advanced retrieval.** Supported native execution paths include selected vector, geo, tensor, and more expressive query features. Availability differs by API and client; see the [capability ledger](docs/CAPABILITIES.md) and [limitations](LIMITATIONS.md).

Search Kernel is intended for applications that want more than basic keyword matching, but do **not** need an independently operated distributed search cluster. Examples include local search over application records, document collections, and single-node retrieval workloads where ranking and structured queries matter.

## Try it in Python

The example below uses the qualified **Python managed-local** mode: the Python client manages a local daemon and persists the index at a local path. It requires the supported Linux platform and the matching 0.1.x release artifacts; follow the [installation guide](docs/getting-started.md) first. Use a fresh `./search-python` directory.

This is the same create → insert → search/filter/sort → close → reopen scenario as the [canonical executable release quickstart](docs/release-quickstart.md#python-managed-local), abbreviated for the README.

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
    results = (
        index.search("quickstart")
        .filter("kind = 'guide'")
        .sort("score", "asc")
        .execute()
    )
    assert [hit.id for hit in results.hits] == ["quickstart-b", "quickstart-a"]

with Index.open(path) as index:
    assert index.get("quickstart-b")["score"] == 1

print("Search and reopen: PASS")
~~~

Expected output when the assertions pass:

~~~text
Search and reopen: PASS
~~~

For the full checked examples—including the additional search-after-reopen assertion—see [Rust embedded, Python, TypeScript/Node, and Ruby quickstarts](docs/release-quickstart.md). **Release qualification is narrower than implementation presence**; the examples do not imply every represented Vespa feature is available in every language.

## Why Search Kernel?

Adding serious search can mean deploying a separate service, even when an application needs only one local index. Vespa already has a sophisticated native engine for indexing, matching, and ranking. Search Kernel makes that engine accessible as an embedded or local-daemon component, while deliberately leaving distributed clustering to Vespa itself.

The goal is to make advanced search **easier to adopt in a single-node application**, not to claim the smallest binary, the lowest memory footprint, or superior query latency. Those properties need workload-specific measurements.

### Choosing the right tool

| Your requirement | Consider | Why |
| --- | --- | --- |
| Search alongside existing SQLite data with a compact dependency | [SQLite FTS5](https://www.sqlite.org/fts5.html) | A straightforward fit when SQLite's full-text facilities meet your needs |
| A Rust-native search library focused on building and querying a text index | [Tantivy](https://github.com/quickwit-oss/tantivy) | A good fit when you want its Rust index model rather than Vespa's engine |
| A local single-node component with supported Vespa search and ranking functionality | **Search Kernel** | Trades a heavier native runtime for access to the embedded Vespa engine |
| Distributed search, replication, or horizontal scaling | [Vespa](https://vespa.ai/) | Search Kernel intentionally does not implement the distributed platform |

These are architectural fit guidelines, **not** benchmark rankings or claims of feature parity.

### Deliberate limits

Search Kernel is not a distributed search system. It does not implement replication, clustering, distributed query execution, a new storage engine, or a new ranking language. It exposes an intentionally bounded subset of Vespa capabilities, and unsupported operations should fail explicitly rather than being silently approximated. See [limitations](LIMITATIONS.md).

## How it works

The direct Rust API reaches Vespa **in-process**, without converting a normal Rust request to gRPC. Non-Rust clients use the local daemon: Python and TypeScript/Node.js use FlatBuffers over gRPC; Ruby uses the supported schema-derived JSON/HTTP edge. Both transport forms meet at the same canonical dispatcher and semantic layer.

~~~mermaid
flowchart TD
    Rust["Rust application<br/>in-process API"] --> IR["Search Kernel IR<br/>shared validation and semantics"]
    Python["Python / TypeScript<br/>local gRPC clients"] --> RPC["FlatBuffers over gRPC"]
    Ruby["Ruby<br/>HTTP client"] --> HTTP["Schema-derived FlatBuffers JSON / HTTP"]
    RPC --> Daemon["searchkerneld<br/>canonical dispatcher"]
    HTTP --> Daemon
    Daemon --> IR
    IR --> Adapter["Vespa backend adapter<br/>Rust / C++ boundary"]
    Adapter --> Engine["Vespa C++ engine<br/>indexing · persistence · matching · ranking"]
~~~

The separation is intentional: the [FlatBuffers contract](api/README.md) represents portable requests, the [owned IR](core/crates/searchkernel-ir/README.md) defines validated meaning, and the [Vespa adapter](core/crates/searchkernel-vespa-engine/README.md) implements that meaning on the pinned engine. Transports must not invent their own query defaults or validation.

[Architecture and design boundaries](docs/architecture.md) · [Guided code walkthrough](docs/reading-the-codebase.md)

## Engineering decisions behind the product

The interesting systems work is not another text matcher; Vespa owns that. The hard part is adapting an existing native search runtime into a developer-facing local component without introducing incompatible semantics or unsafe lifecycle behavior.

**1. Native lifecycle and ownership.** A search engine owns files, threads, and native resources whose lifetimes cannot be treated like ordinary application values. Search Kernel keeps Vespa C++ types behind an adapter, gives Rust explicit ownership/lifecycle responsibilities, and verifies real engine operations including persistence and reopen. See the [native adapter](core/native/vespa/README.md), [Rust native boundary](core/crates/searchkernel-vespa-engine/README.md), and [codebase walkthrough](docs/reading-the-codebase.md).

**2. One meaning for every client.** Independent SDKs are vulnerable to subtly different filtering, validation, defaults, or errors. Search Kernel uses one portable contract and a shared semantic IR; the Rust embedded path and daemon paths converge on that meaning without requiring identical transport mechanics. See [architecture](docs/architecture.md), [semantic resolution](core/crates/searchkernel-ir/README.md), and the [client capability evidence](docs/CLIENT_CAPABILITIES.md).

**3. Tests that can contradict the implementation.** A wrapper can pass unit tests while exercising mocks, a bypass, or different behavior from Vespa. The verification approach uses frozen contracts, upstream behavior as an oracle where applicable, negative cases, and real-engine paths. See the [verification harness](reference/vespa/), [real-engine query tests](core/crates/searchkernel-ir-adapter/src/tests/query_real_engine.rs), and [canonical quickstarts](docs/release-quickstart.md). A documented test strategy is not a claim that every pending release gate is green.

### Development and review approach

Implementation work may be delegated to coding agents, but the [repository workflow](docs/agent-workflow.md) requires bounded tasks, explicit contracts, review, and exact-revision verification. Agent completion claims alone do not establish correctness or release readiness. The architecture, code, and tests remain open for independent review.

## Installation and release scope

**Current qualified release platform: Linux x86-64 with glibc.** The supported 0.1.x beta modes are:

| Client or runtime | Qualified mode |
| --- | --- |
| Rust | Direct, embedded native engine |
| `searchkerneld` | Standalone local daemon |
| Python | Managed-local daemon or connect to an existing daemon |
| TypeScript / Node.js | Connect to an existing daemon over gRPC |
| Ruby | Connect to an existing daemon over HTTP |

Browser gRPC-Web, PHP, Ruby managed-local distribution, Go, macOS, Windows, and Linux ARM64 are **not** qualified release modes, even if source code or partial functionality exists. Public API compatibility is not yet stable. Check the [release support contract](docs/release-support.md), [known limitations](LIMITATIONS.md), and [release notes](docs/release-notes.md).

**To use the beta:** start with [installation and runtime ownership](docs/getting-started.md), select matching artifacts from a release manifest, and follow the [generated executable quickstarts](docs/release-quickstart.md). Current Rust native runtime acquisition may require private release access; see the installation guide rather than assuming a public package registry installation works.

## Building, exploring, and contributing

Application developers should use the release artifacts, not build Vespa from source. Contributors can start with [BUILDING.md](BUILDING.md), including `./dev doctor`, `./dev check-fast`, and the supported native build/toolchain setup. Low-level FlatBuffers versioning, native cache/provenance, regeneration, and CI details live there rather than in this landing page.

For orientation, use [How to read Search Kernel](docs/reading-the-codebase.md); for task submission, branch/PR flow, and verification handoffs, use the [contributor/agent workflow](docs/agent-workflow.md). The [roadmap](docs/roadmap.md) describes intended direction, not what has already been verified; current implementation and support details live in [status](docs/status.md) and the [capability ledger](docs/CAPABILITIES.md).

## License and attribution

Search Kernel is licensed under [Apache License 2.0](LICENSE). It integrates [Vespa](https://vespa.ai/)'s C++ search engine, which remains responsible for the core storage, indexing, query execution, and ranking technology. Search Kernel's wrappers, interoperability layers, and integration tests should not be confused with a replacement search engine.
