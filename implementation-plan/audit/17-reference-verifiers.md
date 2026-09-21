# 17. Frozen conformance corpora and verifier lanes — `reference/`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** Each lane requires `VER` tied to its contract/README; record explicit non-goals. Oracle-based lanes additionally require `ORACLE` provenance.

## 17.1 API conformance — `reference/api-conformance`

- [ ] **SK-17-001** Begin by inventorying the cases that **actually exist**. This snapshot has case families for analyze, component, facet, filter, introspection, query, ranking, schema, search, sort, textual-filter, value, wire, and write; do not extrapolate broader coverage from those directory names.
- [ ] **SK-17-002** Golden outputs are reviewed/frozen independently before implementation changes.
- [ ] **SK-17-003** Adapter/conformance verifier never regenerates expected results during verification.
- [ ] **SK-17-004** Presence-sensitive values (`0`, `false`, empty vs absent) have explicit cases where semantically meaningful.
- [ ] **SK-17-005** Advanced typed-query families are not declared covered merely because `query/` exists. The current query corpus is narrow; weighted/WAND, geo, predicate, sameElement, NN, fuzzy/prefix/substr/suffix and similar execution families require separate focused evidence unless cases are added.
- [ ] **SK-17-006** Advanced update semantics are not declared covered merely because `write/` exists. `UpdateRequest`, conditional mutations, collection/matched-element/tensor-modify paths require explicit cases where supported.
- [ ] **SK-17-007** A single broad V2 search-controls case is not considered comprehensive negative/applicability/collision coverage for QueryModel, PresentationRequest, query profile, search chain, properties, grouping, ranking, and query inputs.
- [ ] **SK-17-008** Rank-profile/schema round trips, recursive result trees, tensor result/value families, and structured error round trips each have explicit cases before this corpus is cited as proof for them.
- [ ] **SK-17-009** Any missing supported semantic family is logged as a conformance-corpus gap, not silently passed from unit/E2E evidence elsewhere.

## 17.2 Standard FlatBuffers JSON evidence

- [ ] **SK-17-010** `reference/json-conformance` is an **empty placeholder in this snapshot** and is marked `N/A / NO EVIDENCE`; it must not be cited as a frozen corpus.
- [ ] **SK-17-011** Current JSON evidence is taken from actual protocol/dispatcher tests (for example `searchkernel-protocol/tests/standard_json.rs`) and is enumerated explicitly.
- [ ] **SK-17-012** If `reference/json-conformance` is activated later, add immutable cases for every supported union/value family, invalid root/union/non-finite/int64 behavior, and JSON -> FlatBuffer -> dispatcher semantic equivalence before counting it as a gate.
- [ ] **SK-17-013** Client-contract fixtures use standard FlatBuffers JSON, never a Search-Kernel-specific JSON dialect.

## 17.3 M0 — Native SDK

- [ ] **SK-17-014** SDK can be consumed outside Vespa source/build tree.
- [ ] **SK-17-015** Open/close/reopen and ABI version are verified.
- [ ] **SK-17-016** Runtime dependencies and exported symbols are verified.

## 17.4 M1 — Rust ownership

- [ ] **SK-17-017** Safe Rust owns/drops native engine exactly once.
- [ ] **SK-17-018** Invalid initialization and repeated lifecycle are covered.
- [ ] **SK-17-019** Binary actually links the native Search Kernel library.

## 17.5 M2 — CRUD/attribute search

- [ ] **SK-17-020** Put/search/update/delete scenario executes through real Vespa.
- [ ] **SK-17-021** No Rust-side fixture matching exists.
- [ ] **SK-17-022** Mutation result semantics match direct C++ oracle.

## 17.6 M3 — Linguistics

- [ ] **SK-17-023** Java oracle is pinned and reproducible.
- [ ] **SK-17-024** Unicode/token/stem/span cases are comprehensive for supported subset.
- [ ] **SK-17-025** Rust output equals frozen oracle output.

## 17.7 M4 — Text indexing/matching

- [ ] **SK-17-026** Real Proton indexing + Matcher path is exercised.
- [ ] **SK-17-027** Term/phrase/BM25/update/delete behavior is verified.
- [ ] **SK-17-028** M3 analysis is actually consumed by both indexing and query paths.

## 17.8 M5 — Persistence/reopen

- [ ] **SK-17-029** Process A mutate/close + Process B reopen/search/update works.
- [ ] **SK-17-030** Persisted fixture metadata records Search Kernel, ABI, Vespa revision, and expected results.
- [ ] **SK-17-031** Crash/recovery behavior is tested separately where claimed.

## 17.9 M6 — Schema/config

- [ ] **SK-17-032** Java config-model oracle is authoritative for selected schemas.
- [ ] **SK-17-033** Generated config equivalence is checked structurally/behaviorally.
- [ ] **SK-17-034** No runtime Java config-model dependency exists.

## 17.10 M7 — Query compilation

- [ ] **SK-17-035** Term/phrase/boolean/equality/range/exists/YQL supported subset reaches real Matcher.
- [ ] **SK-17-036** Unknown field/wrong type/unsupported syntax negative cases are frozen.

## 17.11 M8 — Local results/docsums

- [ ] **SK-17-037** Limit/offset/sort/hits/scores/total hits/stored fields execute through real engine.
- [ ] **SK-17-038** Docsum IDs/fields belong to the exact matched documents.
- [ ] **SK-17-039** Summary projection semantics are verified independently from match semantics.

## 17.12 M9 — Attributes/grouping/facets

- [ ] **SK-17-040** Attribute mutation/sort/filter/ranking access is real-engine verified.
- [ ] **SK-17-041** Facet/group counts and aggregations are compared to Vespa oracle.
- [ ] **SK-17-042** Do **not** overstate `m9-general-grouping-verifier`: its frozen slice is category -> qty recursive grouping with count/average through the private native ABI. Its contract explicitly excludes aliases, keep/filter, ordering, precision, root outputs, stddev, summary hits, continuations, Python, dispatcher, and IR.
- [ ] **SK-17-043** Each excluded grouping feature is backed by its actual later verifier/unit/E2E lane before being marked verified.

## 17.13 M10 — Ranking

- [ ] **SK-17-044** BM25/field features/query features/rank properties/ranking expressions execute in Vespa.
- [ ] **SK-17-045** First-phase and second-phase behavior is directly observable in results/features.
- [ ] **SK-17-046** Two-phase ranking is explicitly exercised by public E2E, not just low-level verifier.
- [ ] **SK-17-047** Rank-profile config generation is regression-tested against required Vespa form.

## 17.14 M11 — Hardening

- [ ] **SK-17-048** Core verifier, sanitizer verifier, and stress verifier all run.
- [ ] **SK-17-049** Lifecycle races, malformed input, repeated reopen, concurrency, leaks, RSS/FD/thread growth are covered.
- [ ] **SK-17-050** Failpoint/error cleanup scenarios are present.

## 17.15 M12 — Dense query tensors and nearest-neighbor execution

- [ ] **SK-17-051** Dense query tensor type syntax, one-indexed-dimension constraint, cell count, finiteness, and native namespace collisions are enforced at the contract-assigned layer rather than duplicated inconsistently.
- [ ] **SK-17-052** Scalar query features and dense tensor query inputs remain distinct; tensor values are never stringified/flattened into rank properties.
- [ ] **SK-17-053** Nearest-neighbor selects the query tensor by the requested name when multiple tensor inputs are present.
- [ ] **SK-17-054** Exact M12 fixture remains HNSW-free and deterministic; expected ordering is checked against the independent direct C++ oracle and the portable adapter path.
- [ ] **SK-17-055** Sparse/mixed/expression tensor inputs remain explicit `Unsupported` at the adapter until a later contract adds them.

## 17.16 M13 — Dense tensor document CRUD

- [ ] **SK-17-056** Dense tensor insert/upsert/get/assign/unset/reassign/delete/reopen use Vespa document/update storage, not a side store.
- [ ] **SK-17-057** Tensor readback preserves canonical type spec and every cell; incompatible scalar/tensor assignments fail deterministically.
- [ ] **SK-17-058** Document tensor updates change subsequent nearest-neighbor ordering through real Vespa execution.
- [ ] **SK-17-059** Sparse/mixed/mapped/cell-level tensor document operations remain explicitly unsupported unless a newer contract supersedes M13.
- [ ] **SK-17-060** Portable M13 path lowers `IndexDefinition` independently rather than reusing the low-level Rust schema helper and thereby hiding schema-adapter defects.

## 17.17 SearchReply specialized verifier — `reference/vespa/searchreply`

- [ ] **SK-17-061** Result IDs, scores, total-hit accuracy, coverage, degradation, native metadata, and result decoding match authoritative native behavior.
- [ ] **SK-17-062** Summary-hit exact IDs and field association are verified where this lane owns the evidence.
- [ ] **SK-17-063** Total-hit-accuracy subcases distinguish exact engine metadata from page-size or returned-hit counts.

## 17.18 Recursive grouping ABI verifier — `reference/vespa/m9-general-grouping-verifier`

- [ ] **SK-17-064** The focused verifier exercises recursive native grouping independently of high-level facade projections.
- [ ] **SK-17-065** Its exact evidence is recorded: recursive group hierarchy, typed group keys/result values, and count/average VALUE outputs for its frozen category -> qty scenario.
- [ ] **SK-17-066** `SK_GROUPING_TREE_OUTPUT_HIT_LIST` existing in the ABI does **not** mean this verifier exercises hit-list payloads; its contract reserves that output kind for later summary support.
- [ ] **SK-17-067** Group iteration ordering is not falsely claimed if the verifier canonicalizes order before comparison.
- [ ] **SK-17-068** M9/Phase 9 reuse of this lane cannot silently skip it when grouping ABI structures change.

## 17.19 Recovery verifier — `reference/vespa/recovery`

- [ ] **SK-17-069** Recovery tests distinguish graceful reopen from abrupt/crash recovery.
- [ ] **SK-17-070** Test state is created fresh for the exact test revision.
- [ ] **SK-17-071** Recovery verifier cannot accidentally pass using old persisted fixture state.

## 17.20 M13 grouping/YQL additive verifier — `reference/vespa/m13/grouping-yql`

- [ ] **SK-17-072** Additive grouping/YQL scenarios execute on top of the M13 tensor-document baseline rather than replacing it.
- [ ] **SK-17-073** Portable YQL/property and grouping combinations are checked against the documented supported subset.
- [ ] **SK-17-074** Unsupported YQL/grouping constructs fail deterministically rather than being partially interpreted.

## 17.21 Phase 9 verifier

- [ ] **SK-17-075** Phase 9 is credited only for what its script runs: M13 cumulative verification, recursive grouping ABI verification, ABI compatibility/freshness checks, focused low-level native grouping/summary decoding, grouping-filter/raw-sort/IN/arithmetic checks, and selected real-Vespa adapter typed-query tests.
- [ ] **SK-17-076** `native_grouping_decodes_quantiles_and_summary_hits` proves the safe-engine/native result seam for grouped summary payloads; it does **not by itself** prove portable IR encoding, dispatcher transport, or language-client decoding.
- [ ] **SK-17-077** Portable nested summary-hit semantics and exact IDs require adapter/public-client E2E evidence in addition to this phase.
- [ ] **SK-17-078** Grouping/attribute/native structure checks cannot be bypassed by stale SDK output.
- [ ] **SK-17-079** Historical Phase 9 prose is not treated as the sole authority for the current V2 portable surface.

## 17.22 Phase 10 verifier

- [ ] **SK-17-080** Public Rust E2E covers the intended capability showcase comprehensively.
- [ ] **SK-17-081** Typed-query, ranking, grouping, tensor, degradation, persistence, and unsupported-gap assertions are explicit.
- [ ] **SK-17-082** The verifier distinguishes "represented", "semantic", "executable", and "ergonomic" support.

## 17.23 Integration verifier — `reference/vespa/i`

- [ ] **SK-17-083** Exercises the public Rust product path against real Vespa after the M13 + Phase 10 baselines.
- [ ] **SK-17-084** Its Rust driver is structurally prevented from importing `searchkernel-ir-adapter`, Vespa engine/bindings, `RecordingBackend`, or `ScriptedBackend`.
- [ ] **SK-17-085** Do **not** credit this verifier with Rust-vs-portable/gRPC equivalence: the current `verify-i.sh` runs a public Rust driver only. Cross-entry-path semantic/outcome equivalence needs a separate gate.

## 17.24 Cumulative verifier — `reference/vespa/verify-through.sh`

- [ ] **SK-17-086** Later native milestones run all required earlier milestone gates.
- [ ] **SK-17-087** `verify-through.sh i` composes the active native baseline and Phase 10 exactly as the verifier map promises.
- [ ] **SK-17-088** The cumulative native verifier is not mistaken for full-product/client verification.
- [ ] **SK-17-089** Client/package/conformance verification has an equally authoritative cumulative gate.

---
