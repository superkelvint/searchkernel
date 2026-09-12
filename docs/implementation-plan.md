# Implementation Plan

Search Kernel is a Rust interface to Vespa's local C++ search engine.

The implementation is deliberately incremental:

```text
define interface
    ↓
define expected behavior
    ↓
build oracle
    ↓
freeze verifier
    ↓
implement
    ↓
verify
```

An implementation agent does not define either the public interface or the expected behavior of the milestone it is implementing.

---

# 1. Rules

## 1.1 Wrap before abstracting

Prefer:

```text
existing Vespa capability
        ↓
small adapter
        ↓
idiomatic Rust representation
```

Do not invent a Search Kernel subsystem unless the underlying Vespa capability genuinely cannot provide what is needed.

---

## 1.2 Replace Java only where required

The embedded product deliberately removes most of Vespa's Java platform.

Replacement work is limited to functionality required to drive the retained C++ engine:

```text
document linguistics
query linguistics
query compilation
schema/config compilation
request/ranking property construction
```

Do not port:

```text
search chains
YQL
HTTP APIs
distributed dispatch
MessageBus
Slobrok
Config Server
Config Proxy
file distribution
```

unless a later milestone explicitly changes scope.

---

## 1.3 Freeze the Rust API before implementation

Every milestone that adds public API must update the immutable API contract first.

The harness contains compile-only programs such as:

```text
reference/vespa/api-contract/src/bin/mN.rs
```

These programs describe the exact API the implementation must provide.

An agent may implement the API.

It may not redesign it.

---

## 1.4 Freeze behavior before implementation

Each milestone is specified by:

```text
CONTRACT.md
scenarios/
expected/
oracle/
verify-mN.sh
```

Expected behavior is created before implementation starts.

The implementing agent must not regenerate expected data.

---

## 1.5 Use the correct upstream oracle

For C++ capabilities:

```text
direct Vespa C++
        vs
Rust wrapper
```

For functionality being replaced from Java:

```text
Vespa Java implementation
        vs
Rust replacement
```

For end-to-end behavior:

```text
normal Vespa behavior
        vs
Search Kernel embedded path
```

---

## 1.6 No mock acceptance path

Mocks may be used in ordinary unit tests.

A milestone verifier must exercise the real path required by that milestone.

For example:

```text
M4 verification

Rust linguistics
    ↓
native ABI
    ↓
real Proton indexing
    ↓
real Matcher
```

A Rust-only text index does not satisfy M4.

---

## 1.7 Normal development must not require Docker

Docker is used to build or regenerate the native SDK and upstream oracles.

The normal development loop remains:

```bash
cargo test --workspace
```

An agent working on ordinary Rust code should not need to enter the Vespa build container.

---

## 1.8 Verification is cumulative

The acceptance command for M7 is conceptually:

```text
verify M0
verify M1
...
verify M7
```

Later milestones must preserve earlier behavior.

---

# 2. Milestone harness

Each milestone should contain:

```text
reference/vespa/mN/
    CONTRACT.md
    SOURCE_TRACE.md

    scenarios/
    expected/

    oracle/
    verify-mN.sh
```

Public API additions are also represented by:

```text
reference/vespa/API_CONTRACT.md
reference/vespa/api-contract/src/bin/mN.rs
```

## CONTRACT.md

Defines:

```text
required behavior
required execution path
explicit non-goals
forbidden shortcuts
```

## SOURCE_TRACE.md

Records the relevant pinned Vespa source path.

For Java-replacement milestones it also identifies the Java implementation being replaced.

Keep this short.

## scenarios/

Small deterministic inputs understandable by inspection.

## oracle/

Produces or documents authoritative upstream behavior.

## expected/

Frozen canonical outputs.

## verify-mN.sh

One acceptance command.

It must never regenerate its own expected results.

---

# 3. Anti-shortcut requirements

The verifier should check structure as well as output.

Where appropriate:

- verify native dynamic linkage;
- verify required native symbols are referenced;
- prevent the fake backend from being compiled into acceptance drivers;
- reject fixture IDs or values appearing in production source;
- reject Vespa headers outside the native adapter;
- reject raw FFI outside the sys crate;
- compile the frozen API fixture;
- verify `reference/vespa/` is unchanged;
- run a separate reviewer looking specifically for bypasses.

For Java-replacement milestones, acceptance must prove the Rust implementation itself produced the required normalized representation.

Calling the Java oracle from production code does not count.

---

# 4. M0 — Native SDK

## Goal

Package the existing working embedded Vespa engine into:

```text
native/sdk/<target>/
```

usable outside the Vespa build environment.

## Required result

The SDK contains:

```text
include/
lib/
share/
manifest.json
```

and exposes the frozen native lifecycle ABI.

## Oracle

The existing successful embedded C++ prototype.

## Verify

Compile a small C program outside the Vespa source tree.

Exercise:

```text
open
close
open
close
```

Check runtime dependencies and ABI version.

## Forbidden shortcuts

- invoking the old gtest as the SDK;
- requiring the Vespa source tree;
- requiring the Vespa development container at runtime;
- exposing Vespa headers publicly.

---

# 5. M1 — Rust ownership

## Goal

Own the M0 engine safely from Rust.

## API

Use the already frozen M1 API.

## Verify

Compile the API contract and exercise:

```text
open
drop
open
drop
```

Also test invalid initialization.

Check that the executable actually links the Search Kernel native library.

## Forbidden shortcuts

- fake Rust engine;
- shelling out to another process;
- raw pointer in public API;
- bypassing the sys/safe-wrapper layering.

---

# 6. M2 — CRUD and attribute search

## Goal

Drive the existing proven document mutation and attribute-search path through Rust.

## Scenario

```text
PUT doc1 attr1=10
PUT doc2 attr1=20

SEARCH 10
    -> doc1

UPDATE doc2 -> 10

SEARCH 10
    -> doc1, doc2

DELETE doc1

SEARCH 10
    -> doc2

SEARCH 20
    -> none
```

## Oracle

The preserved direct C++ embedded fixture.

## Verify

Compare canonical JSON produced by Rust against the frozen expected output.

Verify native mutation/search symbols are actually referenced.

## Forbidden shortcuts

- Rust document database;
- Rust-side integer matching;
- fixture-specific result generation;
- invoking the oracle as the implementation.

M0–M2 remain governed by the verifier package already created.

---

# 7. M3 — Linguistics compatibility

## Goal

Implement the minimum linguistic preprocessing currently supplied outside the C++ search core.

This applies to both documents and queries.

## Initial scope

Start with a deliberately small supported configuration.

For that configuration record:

```text
input string
language
tokens
normalized forms
stemmed/normalized terms
positions / spans where required
```

Index-time output must be sufficient to construct the linguistic annotations consumed by Vespa's C++ indexing path.

Query-time output must produce compatible terms.

## Oracle

Use the pinned Vespa Java linguistics implementation.

Golden generation should run Java directly and emit a small canonical JSON representation.

Example:

```json
{
  "input": "Running cafés",
  "language": "en",
  "terms": [
    {
      "term": "...",
      "start": 0,
      "end": 7
    }
  ]
}
```

The exact representation is established by the oracle before implementation.

## Scenarios

Cover at least:

```text
case normalization
punctuation
multiple whitespace
Unicode text
accented text
stemming
empty input
repeated terms
positions
```

Add languages only when intentionally supported.

## Verify

Run the Rust analyzer against the same fixtures and compare canonical output.

Then compile the API-contract fixture.

## Forbidden shortcuts

- retaining the toy ASCII tokenizer as production behavior;
- calling Java from the Rust runtime;
- using different analysis for query and document without an explicit Vespa-equivalent reason;
- implementing extra synonym/spelling behavior.

## Risk retired

Can Search Kernel produce the linguistic representation Proton expects without the Java runtime?

---

# 8. M4 — Text indexing and matching

## Goal

Connect M3 to the real C++ text index and matcher.

Continue using the known precompiled text configuration fixture.

## Scenario

Use the proven text sequence:

```text
doc1 = "The quick brown fox"
doc2 = "A slow red fox"

term brown
phrase "brown fox"

update doc2
verify old terms disappear

delete doc1
verify its postings disappear
```

Include BM25.

## Oracle

Direct embedded C++ behavior plus M3 Java linguistics goldens.

## Verify

The acceptance path must be:

```text
Rust document
    ↓
M3 analyzer
    ↓
native document annotations
    ↓
Vespa index
    ↓
M3 query analysis
    ↓
Vespa Matcher
```

Compare hits, phrase behavior and ranking where deterministic.

## Forbidden shortcuts

- manually computing matches in Rust;
- manually computing BM25;
- bypassing M3;
- using the original hard-coded spike tokenizer.

## Risk retired

Can the full Java-free text-processing path drive real Vespa indexing and matching?

---

# 9. M5 — Persistence and reopen

## Goal

Verify the wrapped engine as durable local state.

## Scenarios

Process A:

```text
open
put
update
delete
search
close
```

Process B:

```text
open same directory
search
update
search
close
```

Add controlled abrupt termination where practical.

## Oracle

Direct embedded Vespa persistence behavior.

## Persistent compatibility fixture

Record:

```text
Search Kernel revision
native ABI version
Vespa revision
fixture definition
expected results
```

Preserve selected index directories for future upgrade testing.

## Risk retired

Can the embedded wrapper reliably reuse its persisted state?

---

# 10. M6 — Schema to Proton configuration

## Goal

Stop depending on precompiled configuration fixtures for ordinary supported schemas.

## API first

Freeze a minimal schema API.

Do not design every future Vespa field capability.

Initial concepts are only those required for:

```text
text
keyword/string
integer
floating point
boolean if straightforward

index
attribute
stored/summary
```

## Implementation boundary

Conceptually:

```text
Rust schema
    ↓
small Search Kernel compiler
    ↓
configuration consumed by native adapter / Proton
```

The implementation may construct native configuration objects directly if that is cleaner than reproducing Vespa's textual generated-config format.

## Oracle

For a set of simple schemas:

```text
same logical schema
    ↓
Vespa Java config-model
        vs
Search Kernel compiler
```

Canonicalize the comparison around configuration meaning.

Do not require byte-for-byte equality for irrelevant generated details.

Also instantiate `DocumentDB` from both and run behavioral fixtures.

## Scenarios

At minimum:

```text
one text field
one attribute field
text + attribute
multiple scalar types
stored/summary field
invalid configuration
```

## Forbidden shortcuts

- invoking Java config-model at runtime;
- shipping pre-generated config for each fixture as the production implementation;
- implementing `.sd` parsing;
- exposing Vespa generated-config classes through the Rust API.

## Risk retired

Can a Rust application define an index without the Vespa Java configuration platform?

---

# 11. M7 — Query compilation

## Goal

Compile typed Rust queries into the representation consumed by the C++ matcher.

## API first

Freeze the typed query API before implementation.

Initial operators:

```text
Term
Phrase
And
Or
Not
Equals
Range
Exists
```

Do not add a textual query parser.

## Processing

For text operators:

```text
Rust query
    ↓
field metadata from M6
    ↓
M3 linguistic analysis
    ↓
Vespa query representation
```

Numeric and exact operators bypass linguistic analysis as appropriate.

## Oracle

For representative queries, run the equivalent supported query through Vespa's normal preparation path and capture a canonicalized final query representation.

Compare structure and semantics, not Java object identity.

Direct C++ query-builder fixtures may also be used where they provide a clearer oracle.

## Verify

Cover:

```text
single term
phrase
nested AND/OR
NOT
text + numeric filter
range
unknown field
wrong value type
```

Then execute the compiled query through the real matcher.

## Forbidden shortcuts

- YQL parser;
- generic query language;
- Rust-side matching;
- treating every field as text;
- bypassing M3/M6 metadata.

## Risk retired

Can Search Kernel replace the Java query-preparation boundary for its supported typed query API?

---

# 12. M8 — Search request and results

## Goal

Complete the ordinary local request/result path.

## Initial surface

Expose:

```text
limit
offset if supported cleanly
sorting
requested stored fields / summary
scores
total-hit information
```

Use M7 for matching/filtering.

## Summary retrieval

If Vespa's native path separates matching from summary/document retrieval, wrap that native behavior rather than reimplementing it.

There is no distributed fill phase.

## Oracle

Direct C++ local search behavior, with normal Vespa request behavior used when the semantics are prepared outside C++.

## Verify

Deterministic fixtures for:

```text
sort ascending/descending
limit
stored fields
multiple hits
missing values where supported
text + filter + sort
```

## Risk retired

Do we have a complete useful single-node search request without the Java request container?

---

# 13. M9 — Attributes and facets

## Goal

Broaden mutable attribute functionality.

## Scope

Expose practical:

```text
attribute field types
updates
filters
sorting
ranking access
group/facet counts
basic aggregation
```

Do not expose the entirety of Vespa grouping in one milestone.

## Oracle

Direct C++ engine behavior.

## Verify

Include updates without rewriting unrelated text and reopen persistence.

---

# 14. M10 — Ranking integration

## Goal

Expose Vespa ranking configuration and request inputs.

Ranking evaluation remains C++.

## Scope

Initial targets:

```text
BM25
field-specific ranking
attribute features
query features
rank properties
ranking expressions
feature output
```

## Configuration

M6 supplies the configuration route.

Rank assets required by supported profiles must resolve from local files/resources.

Do not recreate Vespa file distribution.

## Oracle

Use Vespa Java/config-model for configuration preparation where relevant and C++ Matcher results for actual evaluation.

## Verify

Fixed documents and queries with:

```text
expected hit order
selected feature values
ranking expressions
query-feature influence
```

Use tolerances for floating-point values where necessary.

## Forbidden shortcuts

- Rust scoring engine;
- RankScript;
- manually computing expected BM25/ranking results.

---

# 15. M11 — Python

## Goal

Expose the established Rust API through PyO3.

Python never talks directly to the native ABI.

## Verify

Run equivalent Rust and Python scenarios and compare canonical results.

No Python-specific search semantics.

---

# 16. M12 — Packaging

## Goal

Install and run Search Kernel without a Vespa or Java development environment.

## Required clean-machine test

The test environment contains no:

```text
Vespa source checkout
Vespa development container
Vespa Java services
Vespa build tools
```

Install the package and run:

```text
create schema
open
put
search
close
reopen
search
```

Record package metadata and native dependencies.

---

# 17. M13 — Hardening

## Native safety

Run:

```text
ASan
UBSan
```

plus appropriate Rust checks.

## State-machine testing

Generate deterministic randomized operation streams:

```text
put
update
delete
search
close
reopen
```

Dump the seed and minimized failing trace.

## Concurrency

Exercise only concurrency actually supported by the underlying engine/API.

Do not invent stronger guarantees.

## Soak tests

Measure:

```text
RSS/PSS
threads
file descriptors
disk growth
latency
native errors
```

---

# 18. Definition of done

Milestone `N` is complete only when:

```text
API contract frozen
        +
oracle frozen
        +
expected outputs frozen
        +
implementation complete
        +
native build succeeds
        +
cargo fmt --check
        +
cargo clippy --workspace --all-targets -- -D warnings
        +
cargo test --workspace
        +
verify M0 ... verify MN pass
```

The implementation agent must not change earlier contracts or verifiers to make new work pass.

Intentional contract changes are separate changes.

---

# 19. Agent workflow

For every milestone:

```text
1. inspect pinned upstream source
2. define public Rust API
3. build upstream oracle
4. define scenarios
5. generate expected outputs
6. build verifier
7. commit/freeze harness

8. implementation agent
9. shortcut-focused reviewer
10. fixer
11. independent verifier
```

The reviewer specifically looks for:

```text
fake backend
fixture-specific logic
Rust reimplementation of Vespa behavior
Java runtime dependency sneaking into production
native library linked but not actually used
API-contract bypass
swallowed errors
unsafe ownership shortcuts
modified verifier/goldens
unnecessary abstractions
```

---

# 20. Immediate work

The existing M0–M2 harness remains valid.

The next harness to design is therefore:

```text
M3 — Linguistics compatibility
```

Its oracle should come from the pinned Vespa Java linguistics implementation.

Only after M3 is frozen should an agent implement Rust linguistics.

Then:

```text
M4 — real text indexing/matching
M5 — persistence
M6 — schema/config compiler
M7 — query compiler
M8 — complete local search request
```

After M8, stop and reassess.

At that point the supported path should be:

```text
Rust API
    ↓
Rust schema/config preparation
Rust document linguistics
Rust query preparation
    ↓
native boundary
    ↓
Vespa C++ persistence/index/match/rank
```

with no production dependency on Vespa's Java request or configuration platform.