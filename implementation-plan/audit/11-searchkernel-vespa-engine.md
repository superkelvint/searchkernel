# 11. Safe low-level Vespa engine — `core/crates/searchkernel-vespa-engine`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + VER`; persistence/concurrency/resource/error-path claims additionally require `HARD` as applicable.

## 11.1 Engine ownership/lifecycle and FFI translation — `engine.rs`, `native.rs`, `build.rs`

- [ ] **SK-11-001** Every native handle has one clear owner and exactly one destruction path.
- [ ] **SK-11-002** Open/create failure cannot leak partially-created handles/resources.
- [ ] **SK-11-003** Drop/close ordering is safe under errors and panics.
- [ ] **SK-11-004** Repeated open/close/reopen is stable.
- [ ] **SK-11-005** Operations cannot race unsafely with close/drop under supported concurrency.
- [ ] **SK-11-006** Native buffers/C strings are copied or consumed before lifetime ends.
- [ ] **SK-11-007** FFI status/error buffers are always freed exactly once.
- [ ] **SK-11-008** The very large `native.rs` translation seam is audited operation-by-operation (documents, updates, queries, tensors, search hits/docsums/features, grouping tree/results) rather than sampled as a single module.
- [ ] **SK-11-009** Build-script SDK discovery/provenance cannot silently rebuild or select a different native SDK than the one recorded as audit evidence.

## 11.2 Documents and updates — `document.rs`

- [ ] **SK-11-010** Put is complete replacement with documented semantics.
- [ ] **SK-11-011** Update assign/clear/arithmetic operations map to native Vespa document updates correctly.
- [ ] **SK-11-012** Delete returns the real Vespa `wasFound` result.
- [ ] **SK-11-013** Field values preserve type and tensor data across the native boundary.
- [ ] **SK-11-014** Unsupported collection/matched-element/tensor-modify updates fail explicitly at the correct layer.
- [ ] **SK-11-015** Conditional mutations fail explicitly until native support exists; conditions are not ignored.
- [ ] **SK-11-016** Update of non-attribute/index-only fields follows verified Vespa behavior rather than stale assumptions.

## 11.3 Schema/config compiler — `schema.rs`

- [ ] **SK-11-017** Generated config is deterministic for equivalent semantic schema.
- [ ] **SK-11-018** Temporary config is installed only after all rendering/validation succeeds.
- [ ] **SK-11-019** Failed config generation does not corrupt a previously valid index/config.
- [ ] **SK-11-020** Text/keyword/integer/float/bool/tensor/position/nested field mapping matches pinned Vespa behavior.
- [ ] **SK-11-021** `index`, `attribute`, and `stored/summary` combinations are generated correctly.
- [ ] **SK-11-022** Rank-profile generated config exactly matches Vespa's required naming and ranking-expression wiring.
- [ ] **SK-11-023** HNSW/ANN field configuration is correct for approximate NN capability.
- [ ] **SK-11-024** Named document summaries are installed only when truly supported; otherwise adapters reject them explicitly.
- [ ] **SK-11-025** File descriptors/temp directories are not leaked across repeated schema generation/open.

## 11.4 Linguistics — `linguistics.rs`, `data.rs`, `stemmer.rs`

- [ ] **SK-11-026** Tokenization matches the pinned Vespa/Java oracle for the supported subset.
- [ ] **SK-11-027** Case normalization, Unicode, punctuation, whitespace, accents, stemming, repeated terms, and empty input match oracle.
- [ ] **SK-11-028** Source byte spans/positions are correct for UTF-8.
- [ ] **SK-11-029** Index-time and query-time analysis are compatible.
- [ ] **SK-11-030** KStem port matches the frozen golden corpus and pinned source behavior.
- [ ] **SK-11-031** No hidden ASCII-only fallback is used in production.

## 11.5 Query model/validation — `query/model.rs`, `query/validate.rs`

- [ ] **SK-11-032** Low-level typed query values express exactly the supported Vespa primitives.
- [ ] **SK-11-033** Field existence/type/searchability/filterability validation is correct.
- [ ] **SK-11-034** Query validation rejects impossible/inconsistent annotations before FFI.
- [ ] **SK-11-035** Recursive query depth/size is bounded sufficiently for hostile inputs.

## 11.6 Native query lowering — `query/native.rs`

- [ ] **SK-11-036** Rust low-level queries map to the correct ABI structures and borrowed lifetimes remain valid through native consumption.
- [ ] **SK-11-037** Term/phrase/boolean/range/exists/IN/typed-query variants preserve exact semantics.
- [ ] **SK-11-038** No Rust-side matching/scoring is used as fallback.
- [ ] **SK-11-039** Query-owned buffers survive long enough and are freed afterward.

## 11.7 YQL — `query/yql.rs`

- [ ] **SK-11-040** Supported reduced grammar is explicitly bounded/documented.
- [ ] **SK-11-041** Unsupported full-Vespa syntax is rejected rather than partially misparsed.
- [ ] **SK-11-042** Parameter/property substitution semantics match the verified subset.
- [ ] **SK-11-043** YQL compilation still reaches real native Matcher execution.

## 11.8 Search/results — `search.rs`

- [ ] **SK-11-044** Limit/offset/sorts/ranking/query features are forwarded correctly.
- [ ] **SK-11-045** Total-hit count/relation are decoded accurately.
- [ ] **SK-11-046** Relevance including infinities is preserved.
- [ ] **SK-11-047** Field/docsum projection corresponds to the requested/default summary path.
- [ ] **SK-11-048** Named summary class selection works only when the class exists and is executable.
- [ ] **SK-11-049** Scalar docsum types decode correctly; rich unsupported docsum values are not silently stringified/dropped.
- [ ] **SK-11-050** Match features decode correct scalar types; opaque tensor feature values are not misrepresented as typed tensors.
- [ ] **SK-11-051** Summary/rank features are populated only when truly available.
- [ ] **SK-11-052** Sorting executes against Vespa; absent sort-value readback is documented rather than fabricated.
- [ ] **SK-11-053** Coverage/degradation fields are sourced from real native metadata.

## 11.9 Tensor — `tensor.rs`

- [ ] **SK-11-054** Dense document tensors serialize/deserialize losslessly.
- [ ] **SK-11-055** Dense query tensors preserve type spec and cell order.
- [ ] **SK-11-056** Shape/cell-count mismatches fail before native use.
- [ ] **SK-11-057** Sparse/mixed tensors fail explicitly where unsupported.
- [ ] **SK-11-058** Tensor memory ownership across FFI is safe.

## 11.10 Hardening/error paths — `hardening.rs`, `error.rs`

- [ ] **SK-11-059** Every native status code maps to an intentional Rust error category.
- [ ] **SK-11-060** Unexpected native failures cannot panic across FFI.
- [ ] **SK-11-061** Failpoints cover cleanup paths, not only happy paths.
- [ ] **SK-11-062** ASan/UBSan runs cover lifecycle, malformed input, search, grouping, ranking, updates, and recovery-critical paths.
- [ ] **SK-11-063** Long-running tests check file descriptors, threads, RSS/PSS, and disk growth.

---
