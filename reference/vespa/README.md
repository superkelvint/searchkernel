# Search Kernel Vespa verifier harness — M0–M2

This package defines the checked-in contracts and acceptance harness for the
first three Search Kernel milestones.

The guiding rule is simple:

> The implementation agent does not define the API and does not define expected behavior.

Both are fixed before implementation begins.

## Milestones

- **M0 — Native SDK:** package the embedded Vespa engine behind a small C ABI.
- **M1 — Rust ownership:** safely open and close that engine from the frozen Rust API.
- **M2 — Document mutation + attribute search:** reproduce the proven embedded Vespa CRUD/search lifecycle through Rust.

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

## Install into the repository

Copy this directory to:

```text
reference/vespa/
```

Then review and commit it **before** giving an implementation milestone to an
agent. That verifier-authoring commit is the specification freeze boundary.

Expected repository shape:

```text
core/crates/
    searchkernel/
    searchkernel-vespa-engine/
    searchkernel-vespa-bindings/

core/native/
    sdk/
        linux-x86_64/
            include/searchkernel_vespa.h
            lib/libsearchkernel_vespa.so
            share/embedded_crud_cfg/...
            manifest.json

reference/
    vespa/
        ...this harness...
```

## Native SDK environment

By default the verifier expects:

```text
core/native/sdk/linux-x86_64
```

Override with:

```sh
export SK_NATIVE_SDK=/absolute/path/to/core/native/sdk/linux-x86_64
```

The SDK must be usable without a Vespa source checkout or Vespa development
container.

## Verify milestones

After the verifier specification is committed:

```sh
./reference/vespa/verify-through.sh m0
./reference/vespa/verify-through.sh m1
./reference/vespa/verify-through.sh m2
```

`m2` is cumulative and therefore runs M0, M1, and M2.

To run one milestone only:

```sh
./reference/vespa/m2/verify-m2.sh
```

Verifier output is written only under:

```text
target/vespa-verifier/
```

The verifier never rewrites golden files.

## API contract

`API_CONTRACT.md` is normative for the public Rust surface through M2.

The API is also enforced mechanically by:

```text
api-contract/src/bin/m1.rs
api-contract/src/bin/m2.rs
```

The verifier runs `cargo check` on these programs. An implementation that
renames methods, changes ownership, or invents a different public model fails
before the behavioral tests run.

This is deliberately a thin wrapper API. In particular, M0–M2 use:

```rust
EngineConfig::from_directory(...)
Engine::open(...)
Document
DocumentUpdate
Value::I32
Query::term(...)
SearchRequest
SearchResult
```

There is no schema builder, query DSL, ranking abstraction, or Search Kernel
analysis system in these milestones.

## Golden behavior

### M0

The native C probe opens and closes the real native engine twice and compares
its normalized JSON result to `m0/expected/01-native-open-close.json`.

M0 is primarily an ABI/lifecycle contract, not a search-result oracle.

### M1

A separate Rust program imports only the public `searchkernel` crate, opens and
closes the real engine twice, and compares normalized output with the M1 golden.

This proves the public Rust ownership path rather than only the raw ABI.

### M2

M2 is based on the already-proven direct C++ embedded Vespa scenario:

```text
PUT doc1 attr1=10
PUT doc2 attr1=20
SEARCH 10 -> doc1
UPDATE doc2 attr1=10
SEARCH 10 -> doc1, doc2
DELETE doc1
SEARCH 10 -> doc2
SEARCH 20 -> none
```

The original proven C++ spike is preserved under:

```text
m2/oracle/embedded_crud_spike.cpp
```

The Rust driver must reproduce the same observable states through the public
Rust API.

## Golden regeneration

Implementation agents must **not** regenerate goldens.

For M2, the direct C++ oracle is the authority. If a maintainer intentionally
changes the pinned Vespa revision or milestone contract, first rebuild and run
the direct C++ oracle, then deliberately update the expected JSON in a separate
verifier-authoring commit.

Do not make `verify-mN.sh` regenerate expected output.

## Preventing shortcuts

The harness uses several layers:

1. **Frozen API compile fixtures** — agents cannot substitute another Rust API.
2. **Frozen expected JSON** — implementation cannot define expected behavior.
3. **Cumulative verification** — later milestones cannot silently regress earlier ones.
4. **Native linkage checks** — M1/M2 executables must dynamically link the Search Kernel native library.
5. **Symbol checks** — the M2 executable must reference the native CRUD/search ABI symbols.
6. **Production-source fixture scan** — known golden document IDs/values may not appear in production Rust code.
7. **Git review** — the checked-in verifier and expected outputs define the
   milestone contract; reference-file changes are reviewed against the
   verifier-authoring commit.
8. **Reviewer pass** — see `AGENT_WORKFLOW.md`; a separate reviewer is explicitly tasked with finding bypasses and reimplementations.

No finite harness can prove that an implementation contains no malicious
hard-coding. The combination of differential/golden behavior, structural
checks, source review, and randomized testing in later milestones is the intended
control.

## Agent workflow

See `AGENT_WORKFLOW.md` for the recommended implement/review/fix/verify loop and
a ready-to-use milestone prompt template.
