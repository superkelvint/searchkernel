# 9. IR -> Vespa adapter — `core/crates/searchkernel-ir-adapter`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + VER`; every claimed executable lowering requires real-engine `VER` or `E2E`; every represented-but-unsupported feature requires `NEG`.

## 9.1 Dependency boundary

- [ ] **SK-09-001** This remains the only crate importing both semantic IR and the Vespa engine crate.
- [ ] **SK-09-002** No public facade types leak into the low-level engine API.
- [ ] **SK-09-003** No Vespa-specific defaults are applied to compensate for unresolved IR.

## 9.2 Schema lowering — `schema.rs`

- [ ] **SK-09-004** Every executable field kind/capability lowers faithfully to low-level Vespa schema/config.
- [ ] **SK-09-005** Rank profiles preserve required generated-config forms, including reserved global second-phase expression wiring.
- [ ] **SK-09-006** Rank profile query inputs/match features/rerank settings lower correctly.
- [ ] **SK-09-007** Unsupported document-summary definitions or field constructs return explicit `Unsupported`.
- [ ] **SK-09-008** The adapter explicitly audits every currently represented rich schema feature it rejects today: schema/document inheritance, fieldsets, search-only fields, imported fields, named document summaries, schema constants, schema ONNX models, profile inheritance/functions/global phase/rank-features/summary-features/sort-features/constants/ONNX/default rank properties/strict/custom properties, and phase controls beyond the native subset.
- [ ] **SK-09-009** Adding native support for one rich schema feature removes only that feature's rejection and adds real-engine regression coverage; broad `schema_feature_not_supported` guards do not accidentally mask newly-supported subfeatures.
- [ ] **SK-09-010** Position/geo derived fields are generated exactly as required by Vespa.
- [ ] **SK-09-011** Schema identity used for reopen remains stable and reflects semantic schema changes.

## 9.3 Search orchestration — `search/mod.rs`

- [ ] **SK-09-012** Query source, filter, sort, ranking, query inputs, generic properties, timeout, summary/projection, grouping validation, and language lowering are composed exactly once in this module family.
- [ ] **SK-09-013** YQL request properties and generic properties reject conflicting duplicate names rather than choosing one source.
- [ ] **SK-09-014** Scalar query inputs, dense tensor query inputs, and raw rank properties preserve distinct semantic namespaces until the exact Vespa request-property collision rules are applied.
- [ ] **SK-09-015** Sort targets that require the raw native sort escape hatch are escaped/serialized without injection or semantic loss; unsupported annotations/missing-value payloads fail explicitly.
- [ ] **SK-09-016** Query-model controls (`query_string`, default index, encoding, filter, language/locale, query-model type, sources/restrict, search path/group) are individually audited for faithful execution or stable explicit `Unsupported`; no broad property copy may make unverified controls appear supported.
- [ ] **SK-09-017** Presentation controls (summary class, fields/projection, highlighting, format, bolding, timing), query-profile/search-chain, generic properties, and other represented controls are individually executed or rejected here with stable codes; none are silently ignored.

## 9.4 Filter lowering — `search/filter.rs`

- [ ] **SK-09-018** Equality/range/boolean/exists lower to the correct native query primitives.
- [ ] **SK-09-019** `IN` lowers to Vespa's native membership primitive where supported, not an accidental OR rewrite.
- [ ] **SK-09-020** Bool/keyword unset behavior matches the verified Vespa contract.
- [ ] **SK-09-021** Unsupported field/filter combinations fail explicitly.
- [ ] **SK-09-022** No filtering is performed in Rust as a fallback.

## 9.5 Typed queries — `search/typed_query.rs`

### Text/boolean
- [ ] **SK-09-023** Term/phrase/proximity/text query lowering preserves linguistic analysis expectations.
- [ ] **SK-09-024** Boolean structure and rank/filter flags survive lowering.

### Weighted/WAND
- [ ] **SK-09-025** Weighted-set, dot-product, WAND, weakAnd weights/target-hits semantics map correctly.
- [ ] **SK-09-026** Signed/range edge cases are tested.

### Annotations/labels
- [ ] **SK-09-027** Supported annotations map exactly to Vespa query item properties.
- [ ] **SK-09-028** Unsupported annotations fail rather than disappear.
- [ ] **SK-09-029** Labels allocate deterministic/internal rank-property names without user collision.

### Geo
- [ ] **SK-09-030** GeoLocation and bounding-box units/coordinate order/derived zcurve field selection are correct.
- [ ] **SK-09-031** Boundary coordinates and invalid ranges are tested.

### Predicate
- [ ] **SK-09-032** Predicate features/range features/composition are encoded exactly as Vespa expects.
- [ ] **SK-09-033** Invalid feature types/ranges fail before native execution.

### Nearest-neighbor
- [ ] **SK-09-034** Exact vs approximate mode maps correctly.
- [ ] **SK-09-035** `targetHits`/exploration parameters and tensor input names are passed correctly.
- [ ] **SK-09-036** Approximate search requires appropriate HNSW/index configuration and fails clearly otherwise.
- [ ] **SK-09-037** Exact fallback is not silently substituted when caller explicitly requested approximate behavior.

### SameElement/nested fields
- [ ] **SK-09-038** Relative member resolution for array<struct> is correct.
- [ ] **SK-09-039** `elementFilter`/member scope does not escape the intended element.

## 9.6 Grouping lowering — `search/grouping.rs`

- [ ] **SK-09-040** Supported grouping tree shapes map faithfully to Vespa grouping operations.
- [ ] **SK-09-041** Arbitrary parallel branches are rejected if the native linear plan cannot represent them.
- [ ] **SK-09-042** Aggregation functions map to the exact native function/argument types.
- [ ] **SK-09-043** Alias/order/output references bind to the intended expression.
- [ ] **SK-09-044** Summary aggregation produces grouped hits with correct IDs, relevance/score, and summary fields.
- [ ] **SK-09-045** Group max vs per-group hit limit distinctions are enforced.
- [ ] **SK-09-046** Unsupported Member/MapLookup/Bucket/function constructs fail explicitly.
- [ ] **SK-09-047** Continuations are rejected explicitly until executable, but preserved semantically before this boundary.

## 9.7 Backend/result mapping — `backend.rs`

- [ ] **SK-09-048** Native hits become canonical `Hit` nodes without losing fields/features/highlights/sort values that are actually available.
- [ ] **SK-09-049** Native grouping hit lists become nested canonical `ResultNode::Hit` children.
- [ ] **SK-09-050** Group child order is preserved.
- [ ] **SK-09-051** Coverage/degradation metadata maps to the correct canonical flags/counters.
- [ ] **SK-09-052** Native failures are translated into structured Search Kernel errors, not generic strings where a stable category exists.
- [ ] **SK-09-053** No result field is fabricated to pretend unsupported native data exists.

---
