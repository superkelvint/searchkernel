# Search Kernel Vespa verifier map

This directory contains the frozen contracts, scenarios, expected outputs,
direct Vespa oracles, and acceptance verifiers for the embedded Search Kernel
wrapper. It is specification material: implementation code must conform to it;
it must not redefine the API or expected behavior.

This README is the authoritative map of the tree. Start here before searching
the individual milestone directories.

## Current status

| Status | Lane | Start here | Acceptance command | Relationship |
| --- | --- | --- | --- | --- |
| **ACTIVE BASELINE** | M14 | [`m14/CONTRACT.md`](m14/CONTRACT.md) | `./dev verify milestone.m14` | Latest cumulative native milestone; includes M0–M13 and the HNSW slice. |
| **ACTIVE INTEGRATION GATE** | I-series | [`i/CONTRACT.md`](i/CONTRACT.md) | `./dev verify product.integration` | Product-path integration gate. Its legacy I-series implementation composes M13 plus Phase 10; M14 remains a separate cumulative native closure. |
| **ACTIVE INTEGRATION SCENARIO** | Phase 10 | [`phase10/verify-phase10.sh`](phase10/verify-phase10.sh) | `./reference/vespa/phase10/verify-phase10.sh` | Canonical public Rust API / real-engine E2E used by the I-series gate. |
| **ACTIVE SPECIALIZED** | Phase 9 | [`phase9/CONTRACT.md`](phase9/CONTRACT.md) | `./reference/vespa/phase9/verify-phase9.sh` | Additive native ABI coverage; not a numbered cumulative M milestone. |
| **ACTIVE SPECIALIZED** | SearchReply | [`searchreply/CONTRACT.md`](searchreply/CONTRACT.md) | `./reference/vespa/verify-through.sh searchreply` | Generic result coverage, degradation, and native metadata behavior. |
| **ACTIVE SPECIALIZED** | Recursive grouping ABI | [`m9-general-grouping-verifier/CONTRACT.md`](m9-general-grouping-verifier/CONTRACT.md) | `./reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh` | Focused low-level grouping verifier, reused by M9 and Phase 9. |
| **ACTIVE SPECIALIZED** | Product recovery | [`recovery/CONTRACT.md`](recovery/CONTRACT.md) | `./reference/vespa/recovery/verify-recovery.sh` | Crash/reopen behavior through the product lifecycle. |
| **ACTIVE ADDITIVE** | M13 grouping/YQL | [`m13/grouping-yql/README.md`](m13/grouping-yql/README.md) | `./reference/vespa/m13/grouping-yql/verify-grouping-yql.sh` | Additional grouping and portable YQL coverage on top of M13. |
| **CUMULATIVE HISTORY** | M0–M13 | The matching `mN/CONTRACT.md` | `./dev verify milestone.mN` | Historical milestones that remain acceptance inputs because later M verifiers are cumulative. |

There is currently no lane classified as obsolete. Age, a numeric milestone
name, or a parallel directory is not evidence that a verifier can be removed.
Retirement requires an explicit maintainer decision, a replacement link, and a
reviewed change to this index.

## Choose a starting point by capability

| If you are working on... | Read first | Then inspect |
| --- | --- | --- |
| Native SDK, handles, startup, or shutdown | [`m0/CONTRACT.md`](m0/CONTRACT.md) | `m0/verify-m0.sh`, [`NATIVE_ABI_CONTRACT.h`](NATIVE_ABI_CONTRACT.h) |
| Rust ownership and the first public engine API | [`m1/CONTRACT.md`](m1/CONTRACT.md) | `m1/verify-m1.sh`, [`API_CONTRACT.md`](API_CONTRACT.md) |
| Basic document mutation and integer search | [`m2/CONTRACT.md`](m2/CONTRACT.md) | `m2/verify-m2.sh` and `m2/oracle/` |
| Linguistics or stemming | [`m3/CONTRACT.md`](m3/CONTRACT.md) | `m3/oracle/`, `m3/kstem/` |
| Text indexing and matching | [`m4/CONTRACT.md`](m4/CONTRACT.md) | `m4/oracle/`, `m4/fixture/` |
| Persistence and low-level lifecycle | [`m5/CONTRACT.md`](m5/CONTRACT.md) | `m5/scenarios/`, `m5/verify-m5.sh` |
| Schema/configuration and attributes | [`m6/CONTRACT.md`](m6/CONTRACT.md) | `m6/oracle/`, `m6/verify-m6.sh` |
| Query trees, YQL, filters, and ranges | [`m7/CONTRACT.md`](m7/CONTRACT.md) | `m7/scenarios/`, `m7/verify-m7.sh` |
| Local requests, sorting, summaries, and results | [`m8/CONTRACT.md`](m8/CONTRACT.md) | `m8/oracle/`, `m8/verify-m8.sh` |
| Attributes, facets, and ordinary grouping | [`m9/CONTRACT.md`](m9/CONTRACT.md) | `m9/verify-m9.sh`, `m9-general-grouping-verifier/` |
| Ranking, BM25, query features, or match features | [`m10/CONTRACT.md`](m10/CONTRACT.md) | `m10/README-HARNESS.md`, `m10/feature-values/` |
| Thread safety, lifecycle hardening, or stress | [`m11/CONTRACT.md`](m11/CONTRACT.md) | `m11/verify-m11-*.sh` |
| Dense query tensors or nearest-neighbor search | [`m12/CONTRACT.md`](m12/CONTRACT.md) | `m12/README-HARNESS.md`, `phase9/` |
| Dense tensor document CRUD and persistence | [`m13/CONTRACT.md`](m13/CONTRACT.md) | `m13/oracle/`, `m13/verify-m13.sh` |
| HNSW approximate nearest-neighbor search | [`m14/CONTRACT.md`](m14/CONTRACT.md) | `m14/oracle/`, `m14/verify-m14.sh` |
| Full public product API behavior | [`i/CONTRACT.md`](i/CONTRACT.md) | `phase10/verify-phase10.sh` and `i/verify-i.sh` |
| SearchReply coverage, degradation, or metadata | [`searchreply/CONTRACT.md`](searchreply/CONTRACT.md) | `searchreply/total-hit-accuracy/` |
| Crash recovery after product operations | [`recovery/CONTRACT.md`](recovery/CONTRACT.md) | `recovery/scenarios/`, `recovery/verify-recovery.sh` |

## Cumulative M-series

The numbered M-series is a chronological, cumulative acceptance chain:

```text
M0 -> M1 -> M2 -> M3 -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> M10 -> M11 -> M12 -> M13 -> M14
```

The latest baseline is M14. A later verifier normally invokes its predecessor,
so M0–M13 are still relevant even though they are not the current feature
frontier. Read the latest contract for the capability being changed, then use
earlier contracts to understand compatibility obligations.

The shared public API compile targets are owned by
`core/crates/searchkernel-vespa-engine/examples/`; active current-implementation
Rust drivers live with their owning crate under `core/crates/`. This tree keeps
the frozen API requirements, scenarios, outputs, and shell/comparison helpers
in [`common/`](common/). The root-level documents are also important:

- [`API_CONTRACT.md`](API_CONTRACT.md) — frozen public Rust API requirements.
- [`NATIVE_ABI_CONTRACT.h`](NATIVE_ABI_CONTRACT.h) — public native ABI boundary.
- [`PINNED_VESPA.json`](PINNED_VESPA.json) — pinned upstream revision metadata.
- [`AGENT_WORKFLOW.md`](AGENT_WORKFLOW.md) — implementation/review workflow.

## How to search without archaeology

Search the small set of orientation files first:

```sh
rg -n '^(#|##)|Goal|Acceptance|Required|Forbidden|Verification' \
  reference/vespa \
  -g 'CONTRACT.md' -g 'README.md' -g 'README-HARNESS.md' -g 'SOURCE_TRACE.md'
```

Then inspect the verifier entrypoint and only the fixtures/oracle files named
by that contract. Expected output and oracle implementation are evidence for a
specific lane, not a general description of the current API.

Avoid treating generated content as source context. In particular, exclude:

```text
target/
__pycache__/
*.pyc
```

The normal cumulative commands are:

```sh
./dev verify milestone.m14                    # current cumulative native baseline
./dev verify product.integration              # current product integration gate
./dev verify vespa.search-reply
```

Phase 9, recovery, and the focused grouping/YQL verifier are intentionally
listed above with their direct commands because they are specialized lanes,
not entries in the cumulative `verify-through.sh` M/I dispatch.

## Contract and verifier rules

Contracts, scenarios, expected outputs, oracle sources, comparators, and
verifier scripts are frozen acceptance artifacts. Do not modify them merely to
make an implementation pass. The implementation must use the real pinned
Vespa path and must not replace it with a Rust matcher, fake backend, fixture
special case, or precomputed result.

The pinned Vespa checkout under `upstream/vespa/` is read-only. Search Kernel
adaptation belongs under `core/native/vespa/`; Vespa types must not leak through
the public Rust or native ABI.
