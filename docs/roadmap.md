# Roadmap

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The project does not reimplement Vespa's search engine. It wraps the existing C++ engine and replaces only the parts of Vespa's Java request/configuration plane that are required to use that engine as an embedded library.

The runtime target is:

```text
Rust API
   │
   ├── document preparation
   ├── query preparation
   └── configuration generation
   │
   ▼
native boundary
   │
   ▼
Vespa C++
   │
   ├── persistence
   ├── indexing
   ├── attributes
   ├── matching
   └── ranking
```

The roadmap is ordered primarily by integration risk.

## Scope boundary

Search Kernel does need replacements for a small set of functionality that normally lives above Proton:

- index-time linguistics;
- query-time linguistics;
- construction of Vespa query trees;
- generation of the Proton configuration required by the supported schema subset;
- search-request and ranking properties.

Search Kernel does **not** initially reproduce:

- YQL;
- Vespa search chains;
- HTTP APIs;
- query profiles;
- spelling correction;
- synonym systems;
- distributed dispatch;
- result merging across nodes;
- MessageBus;
- Slobrok;
- Config Server;
- Config Proxy;
- Java file distribution.

Those may be considered independently later.

---

## M0 — Native SDK

Package the working embedded Vespa prototype as a reproducible native dependency.

Produce a native SDK containing:

```text
include/
lib/
share/
manifest.json
```

It must be usable outside the Vespa development container.

Verify:

- engine startup;
- engine shutdown;
- native ABI version;
- runtime dependencies;
- repeated open/close;
- no Vespa source checkout required by the consumer.

---

## M1 — Rust ownership boundary

Safely own the native engine from Rust.

Target:

```rust
let engine = Engine::open(path, config)?;
drop(engine);
```

Establish:

```text
searchkernel
    ↓
searchkernel-vespa
    ↓
searchkernel-vespa-sys
    ↓
native ABI
```

Verify ownership, destruction, errors, and native safety.

No Vespa C++ type or raw pointer appears in the public Rust API.

---

## M2 — Document mutation and attribute search

Expose the first real Vespa data path through Rust.

Required sequence:

```text
put
search
update
search
delete
search
```

Use the existing integer-attribute fixture and the real Vespa persistence/matching paths.

This milestone proves that non-trivial data can cross the Rust/native boundary.

M0–M2 are already defined by the existing verifier harness and should not be renumbered.

---

## M3 — Linguistics compatibility

Replace the minimum Java linguistics functionality required by the embedded engine.

Cover both sides:

```text
DOCUMENT

raw string
   ↓
tokenize
normalize
stem
annotate positions/terms
   ↓
Vespa C++ indexing


QUERY

raw query term/text
   ↓
tokenize
normalize
stem
   ↓
Vespa query terms
```

The initial scope should remain small.

Automatic language detection, synonyms, spelling correction, and a general analyzer framework are not required.

Use the real Vespa Java linguistics implementation as the golden oracle for the supported subset.

The key invariant is:

> Rust index-time and query-time analysis must produce semantics compatible with normal Vespa analysis.

---

## M4 — Text indexing and matching

Use M3 linguistics to drive the real C++ text path.

Support:

- text fields;
- term queries;
- phrase queries;
- positions;
- BM25;
- updates;
- deletes.

The existing precompiled native configuration fixture may still be used.

This milestone must not retain the temporary toy ASCII tokenizer used by the original spike.

Verify:

```text
Rust analysis
    ↓
annotated Vespa document/query
    ↓
real Proton index/matcher
    ↓
expected results
```

At this point the complete Rust → Vespa text path is proven.

---

## M5 — Persistence and reopen

Verify the wrapped engine as a persistent embedded index.

Exercise:

```text
open
put/update/delete
search
close

open same directory
search
update
close
```

Also establish persistent fixtures for future Vespa-version upgrades.

Test graceful reopen first, then crash/recovery behavior where practical.

Search Kernel does not invent different durability semantics; it documents and verifies what the embedded Vespa engine actually provides.

---

## M6 — Schema to Proton configuration

Remove the dependency on pre-generated Java configuration for the supported schema subset.

Introduce a small Rust schema representation.

Initial field capabilities:

```text
text
string / keyword
integer
floating point
boolean where practical

index
attribute
stored/summary
```

Compile this representation into the configuration required to construct and run the embedded C++ engine.

Conceptually:

```text
Rust Schema
    ↓
Search Kernel config compiler
    ↓
Proton configuration
    ↓
DocumentDB
```

Do not port Vespa's `.sd` language or Java config model wholesale.

Use Vespa's Java config-model output as the oracle for carefully selected fixtures, comparing canonical semantic configuration and resulting behavior rather than requiring identical implementation internals.

---

## M7 — Query compilation

Replace the minimum Java query-preparation path.

Expose a typed Rust query representation.

Initial operators:

```text
term
phrase

AND
OR
NOT

equality
range
exists
```

Compilation is:

```text
Rust Query
    ↓
field-aware query analysis
    ↓
Vespa query representation
    ↓
Matcher
```

M3 linguistics is reused for text terms.

There is no YQL or Vespa textual-query compatibility requirement.

Where possible, compare the resulting normalized query representation against equivalent queries produced by the normal Vespa request path.

---

## M8 — Search request and result integration

Expose the remainder of the ordinary local search request.

Initial targets:

```text
limit
offset where supported
sorting
stored/summary fields
result hits
scores
total-hit information available from the engine
```

Support structured filtering through the M7 query representation.

Wrap the local summary/docsums path where required to return stored document fields.

Do not reproduce distributed search dispatch or distributed result merging.

---

## M9 — Attributes, grouping, and facets

Broaden the attribute support initially proven in M2.

Cover useful mutable attribute types and operations:

```text
attribute updates
filtering
sorting
ranking access
facet/group counts
basic aggregations where practical
```

Expose Vespa capabilities rather than designing a separate aggregation engine.

---

## M10 — Ranking integration

Expose Vespa's existing ranking system.

Initial targets:

```text
BM25 configuration
field-specific ranking
attribute features
query features
rank properties
native rank features
ranking expressions
feature inspection
```

Ranking execution remains entirely in Vespa C++.

Search Kernel does not implement RankScript or another ranking VM here.

For ranking assets such as constants or models, embedded operation should use explicit local resources rather than recreating Vespa's file-distribution service.

---

## M11 — Python

Expose the established Rust API through PyO3.

The dependency direction remains:

```text
Python
   ↓
PyO3
   ↓
searchkernel
   ↓
native engine
```

Python does not call the native ABI directly.

Rust and Python fixtures must produce equivalent results.

---

## M12 — Packaging

Make Search Kernel installable on a machine without:

```text
Vespa source
Vespa development container
Vespa build tools
Java runtime
```

Produce the first supported native bundle and Python wheel.

Record:

```text
Search Kernel version
native ABI version
pinned Vespa revision
target platform
```

Package size should be measured before aggressively optimizing it.

---

## M13 — Hardening

Once the capability path is complete, harden it.

Test:

- concurrent operations;
- lifecycle races;
- malformed input;
- native failures;
- crash/recovery;
- repeated reopen;
- long-running mutation/search workloads;
- memory safety;
- resource leaks.

Measure:

- startup time;
- RSS/PSS;
- thread count;
- file descriptors;
- package size;
- indexing throughput;
- query latency.

Run native sanitizers routinely.

---

## Verification strategy

Different milestones have different authorities.

```text
M0–M2    direct Vespa C++ behavior

M3       Vespa Java linguistics

M4–M5    direct embedded C++ engine

M6       Vespa Java config-model + C++ behavior

M7       Vespa query preparation / canonical query behavior

M8–M10   direct C++ behavior, with Java oracle where Java currently
          defines request/config preparation
```

Java may therefore remain a **test/oracle dependency**.

It must not become a Search Kernel runtime dependency.

---

## Checkpoints

### After M4

We have proven:

```text
Rust
  ↓
Rust linguistics
  ↓
native boundary
  ↓
Vespa text indexing + matching
```

using a pre-generated configuration fixture.

This is the first major architecture checkpoint.

### After M8

We have:

```text
Rust schema
Rust document preparation
Rust query preparation
        ↓
Vespa C++ engine
```

for the supported subset.

At this point an application should be able to create and search an index without requiring the Vespa Java runtime or configuration platform.

This is the second major checkpoint.

---

## Vespa upgrades

Vespa remains pinned.

Updating Vespa is an explicit compatibility exercise, not routine dependency churn.

Every upgrade must verify:

- C++ adapter compatibility;
- native ABI behavior;
- Java-replacement parity where relevant;
- search behavior;
- persisted-index compatibility.

Remaining on the current revision is always a valid outcome.

---

## Later

Possible later work includes:

- richer linguistics;
- automatic language detection;
- synonyms;
- textual query parsing;
- vectors and HNSW;
- tensors;
- ONNX/model ranking;
- richer grouping;
- RankScript;
- additional language bindings;
- dependency/runtime slimming.

None are required to establish the initial embedded wrapper.