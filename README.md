# Search Kernel

Search Kernel is an embedded search engine.

The goal is to provide high-quality full-text search, structured filtering, ranking, and live updates without requiring a separate search service.

Search Kernel is designed to be used like a database library:

```python
from searchkernel import Index

index = Index.open("./search")

index.put({
    "_id": "1",
    "title": "The quick brown fox"
})

results = index.search("brown fox")
```

## Goals

Search Kernel is built around a few ideas:

- **Embedded.** Search runs with the application, not as a separate cluster.
- **Simple to operate.** An index should be a directory, not infrastructure.
- **Mutable.** Inserts, updates, and deletes are normal operations.
- **Durable.** Indexes survive restarts and recover cleanly.
- **Efficient.** Large indexes should make good use of SSD and bounded memory.
- **Evolvable.** Schemas and index representations should be able to change over time.
- **Extensible ranking.** Ranking should support both conventional relevance and application-specific signals.
- **Good APIs.** Rust is the primary interface, with first-class Python support.

## Scope

Search Kernel is focused on single-node application search.

It is not a distributed search engine and does not intend to become one.

For workloads that require clustering, replication, or distributed query execution, existing distributed search systems are a better fit.

## Status

Search Kernel is under active development.

The current focus is the core embedded workflow:

```text
open
index
search
update
delete
reopen
```

Performance, memory use, durability, and operational simplicity are treated as things to measure, not assumptions.