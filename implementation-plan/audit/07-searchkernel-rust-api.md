# 7. Public Rust product API — `core/crates/searchkernel`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT/CONF`; every capability claimed executable through the public Rust API additionally requires `E2E`.

## 7.1 Schema facade — `schema.rs`, `portable_definition.rs`, schema-fidelity tests

- [ ] **SK-07-001** Rust schema helpers produce the same resolved semantics as equivalent portable definitions for every concept they claim to expose.
- [ ] **SK-07-002** Rust ergonomic schema is not falsely treated as the whole V2 schema; portable-only rich schema constructs remain available through canonical definition bytes/low-level models and preserve round-trip fidelity.
- [ ] **SK-07-003** Primitive/tensor field helpers encode the expected capabilities and reject invalid combinations.
- [ ] **SK-07-004** Rank profiles preserve first/second phase, rerank count, match features, and declared query inputs.
- [ ] **SK-07-005** Portable definition used for manifest provenance is lossless for fields it claims to preserve.
- [ ] **SK-07-006** Manifest-provenance encoding is not used as an alternate runtime semantic path.

## 7.2 Index lifecycle — `index/mod.rs`, `index/native.rs`, `index/manifest.rs`

- [ ] **SK-07-007** `create` fails safely on invalid definition/path/state.
- [ ] **SK-07-008** `open` validates manifest/schema/native compatibility before exposing an index.
- [ ] **SK-07-009** `close`/drop are idempotent as intended and never double-free native resources.
- [ ] **SK-07-010** Operations after close fail deterministically.
- [ ] **SK-07-011** Create/open/reopen preserve durable semantics.
- [ ] **SK-07-012** Multiple index handles/runtimes obey documented ownership and concurrency rules.
- [ ] **SK-07-013** Manifest writes are atomic enough to avoid accepting partially-written state.
- [ ] **SK-07-014** Manifest/native index mismatch fails closed.
- [ ] **SK-07-015** Failed create/open cleans partial temporary state.

## 7.3 Documents — `document.rs`

- [ ] **SK-07-016** Serde conversion preserves supported scalar/list/object/tensor values without silent coercion.
- [ ] **SK-07-017** `_id` handling is deterministic and cannot conflict ambiguously with explicit method IDs.
- [ ] **SK-07-018** Get projection distinguishes absent/empty/specified projection.
- [ ] **SK-07-019** Missing GET returns the documented optional result rather than an unrelated error.

## 7.4 Patch/update — `patch.rs`

- [ ] **SK-07-020** Patch `set` and `unset` remain distinct from setting null/false/zero.
- [ ] **SK-07-021** Multiple patch operations retain deterministic order if order is semantically relevant.
- [ ] **SK-07-022** Unsupported advanced update operations are not falsely exposed as working Rust conveniences.
- [ ] **SK-07-023** Mutation errors preserve canonical error codes/details.

## 7.5 Filters — `filter.rs`

- [ ] **SK-07-024** Typed Rust filter helpers resolve through canonical IR validation.
- [ ] **SK-07-025** Typed helper `IN` remains first-class.
- [ ] **SK-07-026** Boolean helper composition produces the same IR as equivalent portable structured filters.
- [ ] **SK-07-027** Rust facade does not introduce independent field/type/operator semantics.

## 7.6 Query DSL — `query.rs`

- [ ] **SK-07-028** Each public helper maps to exactly one intended canonical query node.
- [ ] **SK-07-029** Phrase/proximity/boolean/weighted/WAND/fuzzy/prefix/substr/suffix/sameElement/geo/predicate/NN helpers preserve annotations and values.
- [ ] **SK-07-030** Duplicate/conflicting annotations are rejected consistently.
- [ ] **SK-07-031** Nearest-neighbor uses named query tensor input rather than duplicating tensor values inline.
- [ ] **SK-07-032** Query labels cannot collide with internal adapter properties.

## 7.7 Search builder — `search.rs`

- [ ] **SK-07-033** `search`, `browse`, and typed `query` preserve distinct source semantics.
- [ ] **SK-07-034** Builder methods only collect caller intent; semantic validation stays in IR.
- [ ] **SK-07-035** Limit/offset are range-checked.
- [ ] **SK-07-036** Sorting preserves declaration priority.
- [ ] **SK-07-037** Missing-value sort modes preserve explicit zero/false values and never use truthiness defaults.
- [ ] **SK-07-038** Search-field duplicates/weights behave exactly as documented.
- [ ] **SK-07-039** Ranking/input/property namespaces remain separate.
- [ ] **SK-07-040** Query-model and presentation controls exposed by the Rust facade lower into the same canonical semantic fields as portable requests; facade omission of a represented control is documented as an ergonomic gap, not silently emulated via `.property()`.
- [ ] **SK-07-041** Query profile/search chain/generic property controls cannot bypass central applicability and collision validation.
- [ ] **SK-07-042** Builder execution always routes through `resolve_search`.
- [ ] **SK-07-043** No direct facade -> Vespa execution bypass exists.

## 7.8 Grouping — `grouping.rs`

- [ ] **SK-07-044** Public grouping helpers map losslessly to canonical grouping IR for the subset they expose.
- [ ] **SK-07-045** Nested groups preserve order/structure.
- [ ] **SK-07-046** Aggregation aliases and order-by references resolve correctly.
- [ ] **SK-07-047** Summary aggregation can preserve nested hit children in public results.
- [ ] **SK-07-048** Unsupported expression families are rejected or absent from the facade rather than approximated.

## 7.9 Analyze — `analyze.rs`

- [ ] **SK-07-049** Exactly one valid analysis target (field or analyzer) is required.
- [ ] **SK-07-050** Analyze uses backend/native linguistics; the facade never tokenizes independently.
- [ ] **SK-07-051** Token surface/term/position/start/end spans survive public conversion.

## 7.10 Tensor facade — `tensor.rs`

- [ ] **SK-07-052** Dense tensor type spec and cell count/shape are validated.
- [ ] **SK-07-053** Numeric conversion does not silently narrow values.
- [ ] **SK-07-054** Unsupported sparse/mixed public tensor paths fail explicitly.
- [ ] **SK-07-055** Tensor document/query-input round trips preserve exact values across reopen where supported.

## 7.11 Result conversion — `result.rs`

- [ ] **SK-07-056** Public Rust results do not destructively drop canonical result-tree nodes.
- [ ] **SK-07-057** Nested group hits survive conversion and remain attached to the correct group.
- [ ] **SK-07-058** Child ordering is preserved.
- [ ] **SK-07-059** Infinite relevance is representable.
- [ ] **SK-07-060** Match/summary/rank features remain distinct.
- [ ] **SK-07-061** Bytes are not inferred from integer lists.
- [ ] **SK-07-062** Dense/sparse/mixed tensor result values are decoded without changing tensor kind.
- [ ] **SK-07-063** Coverage/degradation reasons are preserved.
- [ ] **SK-07-064** Structured search-result errors are preserved separately from operation errors.
- [ ] **SK-07-065** Projection into caller `T` fails with a useful structured error when fields cannot deserialize.

## 7.12 Error surface — `error.rs`

- [ ] **SK-07-066** Public errors preserve canonical machine-readable codes/details.
- [ ] **SK-07-067** Error mapping never requires users to parse messages for semantics.
- [ ] **SK-07-068** Native/adapter/portable distinctions are translated consistently without losing root cause.

## 7.13 Rust real-engine E2E example

- [ ] **SK-07-069** `try_searchkernel_client_api_e2e.rs` exercises all capabilities claimed ergonomic in Rust.
- [ ] **SK-07-070** It explicitly covers two-phase ranking when supported.
- [ ] **SK-07-071** It explicitly covers typed query variants that are claimed executable.
- [ ] **SK-07-072** It covers grouping + summary hits and recursive result decoding once public.
- [ ] **SK-07-073** It covers tensor readback/query inputs/NN and approximate/exact ANN semantics when supported.
- [ ] **SK-07-074** It covers reopen/persistence and post-reopen semantic equivalence.
- [ ] **SK-07-075** It clearly lists unsupported public gaps rather than silently omitting them.

---
