# M4 — Text indexing and matching

M0–M3 are complete and must not regress.

## Goal

Connect M3 linguistics to the real embedded Vespa text indexing and Matcher path.

The acceptance path is normative:

```text
Rust raw document text
    -> M3 document analysis
    -> Search Kernel native ABI
    -> Vespa document linguistic annotations
    -> real Proton text index

Rust raw query text
    -> M3 query analysis
    -> Search Kernel native ABI
    -> Vespa query tree
    -> real Proton Matcher
    -> Vespa ranking/BM25
```

M4 proves the complete Java-free Rust -> Vespa lexical path.

## Supported scope

Exactly:

- one precompiled indexed text field, named `title` by the frozen fixture;
- put text document;
- replace/update the text value of an existing document;
- delete through the already-supported M2 mutation path;
- single analyzed term query;
- analyzed phrase query;
- phrase positions;
- a real Vespa BM25 rank profile;
- hit ordering produced by Vespa;
- immediate visibility after put/update/delete as provided by the embedded path.

Not in M4:

- general schema/config compilation (M6);
- arbitrary boolean/filter query compilation (M7);
- pagination, projections, docsums, total-hit API, general result model (M8);
- facets/grouping (M9);
- general rank-profile/rank-property configuration (M10);
- Rust-side lexical matching or scoring.

## Authority

M4 behavioral authority is a standalone direct C++ oracle using the pinned Vespa
`DocumentDB`, persistence path, text index and `Matcher` with the same frozen
text configuration fixture used by M4.

The direct C++ oracle must not call the Search Kernel Rust crate or Search Kernel
native adapter.

M3 linguistic authority remains the frozen/live M3 Java oracle. During harness
authoring only, M3 Java output is used to supply analyzed terms/positions to the
direct C++ oracle. The C++ oracle itself does not tokenize or stem.

## Frozen scenario

The scenario deliberately distinguishes:

- term matching from phrase matching;
- phrase adjacency from simple AND semantics;
- update replacement from additive postings;
- delete from stale postings;
- M3 normalization/stemming integration;
- real Vespa BM25 ranking.

The corpus includes:

```text
doc1 = "The quick brown fox"
doc2 = "A slow red fox"
doc3 = "brown very fox"      # both terms, not a phrase
doc4 = "fox fox fox fox"     # BM25/term-frequency probe
doc5 = "Running foxes"       # M3 integration probe
```

The exact sequence is in `scenarios/cases.json`.

## Phrase invariant

For the initial state:

```text
term brown
    may match doc1 and doc3

phrase "brown fox"
    must match doc1
    must NOT match doc3
```

This is the primary positions proof. A backend that implements phrase as
`brown AND fox` fails M4.

## Update invariant

After replacing doc2 with:

```text
A slow brown fox
```

old term `red` must no longer match doc2 and phrase `brown fox` must match it.
The update must replace native postings; it must not append a second logical
version in Rust.

## Delete invariant

After deleting doc1:

- `quick` must no longer match;
- `brown fox` must no longer return doc1;
- no stale doc1 posting may remain visible.

## M3 integration invariant

At least one scenario uses raw input whose equality depends on M3 analysis rather
than byte-for-byte text equality. The frozen scenario uses document text
`Running foxes` and raw query `RUNNING`.

The Rust M4 driver must pass raw strings to the public M4 API. It must not read
pre-analyzed terms from the M4 verifier.

## BM25 invariant

The frozen text fixture must contain a real Vespa rank profile that evaluates
BM25 for `title`.

The direct C++ oracle records Vespa hit order and, when available through the
oracle path, relevance scores for the `bm25-fox-initial` checkpoint.

Score comparison uses a small numeric tolerance; the verifier never computes
BM25 independently.

If the supported Rust M4 surface intentionally does not expose raw scores yet,
the acceptance driver may compare BM25 ranking order while a direct native test
asserts that the configured ranking expression is the real Vespa BM25 feature.
Do not add a Rust scoring implementation merely to make scores observable.

## Public M4 surface

The frozen compile fixture defines only the small low-level text API needed for
this milestone. M4 must not introduce a general query framework ahead of M7.

The intended additive surface is conceptually:

```rust
TextDocument { id, field, text }
TextQuery::Term { field, text }
TextQuery::Phrase { field, text }
TextSearchResult { document_ids }

Engine::put_text(...)
Engine::update_text(...)
Engine::search_text(...)
```

Deletion reuses the previously committed M2 mutation API rather than creating a
text-specific delete subsystem.

The compile fixture is normative if syntax differs from this prose.

## Forbidden shortcuts

Production code must not:

- match terms or phrases in Rust;
- calculate BM25 in Rust;
- use a Rust in-memory/search index as the M4 acceptance path;
- bypass M3 document/query analysis;
- retain or call the original toy ASCII/whitespace tokenizer;
- read `reference/vespa/m4/expected` or scenario files at runtime;
- branch on scenario ids, document ids, query strings or expected results;
- invoke the direct C++ oracle as the implementation;
- patch tracked files in `upstream/vespa`;
- introduce a dependency from `searchkernel-vespa-engine` to `searchkernel-ir`.

## Verification

After the harness is frozen, `verify-m4.sh` proves:

1. M3 cumulative verification passes;
2. the frozen M4 API contract compiles;
3. fmt/clippy/workspace tests pass;
4. the Rust scenario driver uses the real `searchkernel-vespa-engine` engine path;
5. Rust results match direct-C++ frozen goldens;
6. phrase-position checks pass;
7. update and delete visibility checks pass;
8. M3-sensitive raw text/query matching passes;
9. BM25 ordering/score behavior matches the direct C++ oracle;
10. structural shortcut checks pass;
11. the frozen harness checksum is unchanged.
