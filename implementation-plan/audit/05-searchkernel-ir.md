# 5. Semantic core — `core/crates/searchkernel-ir`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT/CONF + NEG`; every represented semantic family needs round-trip or canonical-IR evidence. Backend execution is not proven here.

## 5.1 Owned model — `src/model/*`

### Values
- [ ] **SK-05-001** `ValueIr` distinguishes null/boolean/string/signed/unsigned/float/bytes/list/object/tensor without lossy coercion.
- [ ] **SK-05-002** Int64/UInt64 boundaries and overflow behavior are exact.
- [ ] **SK-05-003** Non-finite values are accepted/rejected only where the portable contract permits them.
- [ ] **SK-05-004** Dense/sparse/mixed tensor representations preserve type spec, addresses/blocks, and values.

### Schema
- [ ] **SK-05-005** Field kinds/capabilities are explicit after resolution.
- [ ] **SK-05-006** Analyzer identity/version is explicit.
- [ ] **SK-05-007** Search/index/attribute/stored/summary semantics are explicit.
- [ ] **SK-05-008** Nested struct/array/map/weighted-set/reference/predicate/position/tensor metadata survives semantic resolution where represented.
- [ ] **SK-05-009** Rich index-definition families (inheritance, structs, fieldsets, search-only/imported fields, document summaries, rank profiles, constants, ONNX models) survive losslessly through semantic representation even when backend execution is unsupported.
- [ ] **SK-05-010** Rank profiles and document summaries survive losslessly.

### Filters
- [ ] **SK-05-011** `IN` remains a first-class IR node.
- [ ] **SK-05-012** Range bounds preserve inclusive/exclusive and open-ended semantics.
- [ ] **SK-05-013** Boolean normalization does not change truth conditions.
- [ ] **SK-05-014** Missing/unset semantics are explicit where required rather than inferred by backends.

### Queries
- [ ] **SK-05-015** Every advertised typed-query variant has an owned IR representation.
- [ ] **SK-05-016** Query annotations are preserved losslessly and validated centrally.
- [ ] **SK-05-017** Labels cannot collide with adapter-owned/internal properties.
- [ ] **SK-05-018** Nearest-neighbor requests preserve tensor input name, target hits, approximate/exact control, and related annotations.
- [ ] **SK-05-019** Weighted-set/WAND/dot-product weights preserve signedness/range/order semantics.
- [ ] **SK-05-020** Geo/predicate/sameElement/userInput/userQuery representations preserve caller meaning.
- [ ] **SK-05-021** Query-model state is a first-class semantic family: query string/default index/encoding/filter/language/locale/type/sources/restrict/search path/search group are not flattened into an untyped property bag.
- [ ] **SK-05-022** Presentation state is a first-class semantic family: summary/projection/highlighting/format/bolding/timing preserve presence and applicability.
- [ ] **SK-05-023** Query profile, search chain, generic query properties, rank properties, and query inputs remain distinct namespaces.

### Grouping
- [ ] **SK-05-024** Recursive grouping operations preserve branch ordering and structure.
- [ ] **SK-05-025** Aggregation expressions and aliases are preserved.
- [ ] **SK-05-026** Summary aggregation remains distinct from scalar aggregation.
- [ ] **SK-05-027** Continuation tokens are opaque and preserved.
- [ ] **SK-05-028** Raw grouping source remains unresolved caller intent and cannot masquerade as resolved grouping IR.

### Ranking
- [ ] **SK-05-029** Rank profile selection, rank properties, query inputs, expression inputs, and feature requests are distinct.
- [ ] **SK-05-030** Tensor query inputs retain exact tensor shape/value representation.
- [ ] **SK-05-031** First/second-phase rank profile expressions, rerank count, match features, and query inputs survive schema round trips.

### Results
- [ ] **SK-05-032** Canonical result is a heterogeneous recursive tree.
- [ ] **SK-05-033** Nested hits inside groups are preserved.
- [ ] **SK-05-034** Result groups preserve id/label/value/fields/children/continuations/source/types/relevance/limits/metadata.
- [ ] **SK-05-035** Hits preserve id/relevance/fields/highlights/source/types/match/summary/rank features/sort values.
- [ ] **SK-05-036** Coverage/degradation/timing/trace/errors/metadata remain separate structured fields.

## 5.2 Portable/API -> IR — `src/api_to_ir/*`

### Document/write conversion
- [ ] **SK-05-037** Insert vs upsert semantics remain distinct.
- [ ] **SK-05-038** Patch set/unset preserves explicit unset rather than conflating it with null/false/missing.
- [ ] **SK-05-039** Advanced update operations preserve order and target path where represented.
- [ ] **SK-05-040** Conditional mutation intent is preserved through IR even when backend execution is unsupported.
- [ ] **SK-05-041** Unsupported represented update variants are rejected explicitly, never silently dropped.
- [ ] **SK-05-042** Get projection distinguishes absent, empty, and explicit field lists.

### Schema conversion
- [ ] **SK-05-043** Portable schema resolves to the same semantic schema as equivalent direct Rust schema input.
- [ ] **SK-05-044** Duplicate fields/profile names/summary names are rejected deterministically.
- [ ] **SK-05-045** Invalid field capability combinations fail centrally.
- [ ] **SK-05-046** Rank profiles are validated consistently at both Rust and FlatBuffers ingress, including mutually-exclusive phase controls, required expressions, duplicate names, typed inputs/defaults, constants, ONNX model mappings, feature lists, functions, and properties.
- [ ] **SK-05-047** Describe/index-definition round trips are lossless for all represented fields.

### Filter conversion
- [ ] **SK-05-048** Structured filters validate field existence, filterability, type compatibility, and operator legality.
- [ ] **SK-05-049** Textual filter clauses are parsed independently and retain clause-local bindings.
- [ ] **SK-05-050** Missing, unused, duplicate, and wrong-clause bindings fail deterministically with clause/path information.
- [ ] **SK-05-051** Same binding name may be reused independently across clauses.
- [ ] **SK-05-052** Textual and equivalent structured filters resolve to equal canonical IR.

### Query conversion
- [ ] **SK-05-053** Every represented query union discriminator maps to the intended IR node.
- [ ] **SK-05-054** Unknown/unimplemented variants fail explicitly.
- [ ] **SK-05-055** Typed query field compatibility is validated centrally.
- [ ] **SK-05-056** Query annotation values/types are validated centrally.
- [ ] **SK-05-057** Search text, browse/match-all, typed query, and YQL remain distinct query-source semantics.

### Grouping conversion
- [ ] **SK-05-058** Facet conveniences normalize into canonical grouping rather than a parallel facet engine.
- [ ] **SK-05-059** Root aggregations normalize into canonical grouping.
- [ ] **SK-05-060** Group aliases and output references validate correctly.
- [ ] **SK-05-061** Raw grouping source is rejected until normalized, rather than executed as unresolved semantics.
- [ ] **SK-05-062** Continuation presence is preserved even if native execution later rejects it.

### Ranking conversion
- [ ] **SK-05-063** Profile existence is validated against the resolved schema.
- [ ] **SK-05-064** Rank properties, query properties, and inputs cannot overlap silently.
- [ ] **SK-05-065** Adapter-reserved/internal rank property namespaces are rejected at semantic ingress.
- [ ] **SK-05-066** Query input scalar/tensor types are validated against profile declarations where specified.

### Search resolver — `resolve.rs` / `search.rs`
- [ ] **SK-05-067** Direct Rust and portable search ingress call the same complete resolution operation.
- [ ] **SK-05-068** Defaults are applied once and only here.
- [ ] **SK-05-069** `search(whitespace)` vs `browse()` behavior matches the facade contract.
- [ ] **SK-05-070** Limit/offset/page-derived values are range-checked and overflow-safe.
- [ ] **SK-05-071** Duplicate search fields/sorts/facets/aliases are handled according to contract.
- [ ] **SK-05-072** YQL-incompatible high-level controls fail deterministically.
- [ ] **SK-05-073** Query-model applicability/defaults are resolved centrally, including `query_string`, default index, language/locale, source/restrict, search path/group, and query-model type controls.
- [ ] **SK-05-074** Presentation controls are resolved centrally and preserve absent vs explicit empty/false values.
- [ ] **SK-05-075** `query_profile` / `search_chain` remain explicit even when the embedded backend will reject them; they are never silently converted to generic properties.
- [ ] **SK-05-076** Typed controls and generic properties that target the same native property cannot silently overwrite one another; collision behavior is deterministic.
- [ ] **SK-05-077** Raw sort/grouping cannot enter resolved IR unless normalized.
- [ ] **SK-05-078** Unsupported represented controls remain represented/semantic as intended and are not erased.

### Value conversion
- [ ] **SK-05-079** FlatBuffer values convert to owned IR without narrowing.
- [ ] **SK-05-080** JSON-originated request values follow the exact same type semantics as FlatBuffer-originated values.
- [ ] **SK-05-081** Bytes are distinguished from lists of small integers.
- [ ] **SK-05-082** Sparse/mixed tensors are never coerced into dense tensors.

## 5.3 Textual-filter integration — `filter_expression.rs`

- [ ] **SK-05-083** Binding is schema-aware and parser-independent.
- [ ] **SK-05-084** Integer fields reject fractional literals/bindings.
- [ ] **SK-05-085** Float fields normalize integer literals safely to float where specified.
- [ ] **SK-05-086** Numeric overflow/non-finite values fail deterministically.
- [ ] **SK-05-087** Keyword/text/bool operator legality matches the filter-language specification.
- [ ] **SK-05-088** Empty `IN` is rejected.
- [ ] **SK-05-089** IN-value and binding-count limits are enforced after binding expansion as well as parse time.
- [ ] **SK-05-090** Error source spans survive semantic binding.

## 5.4 IR -> portable API — `src/ir_to_api/*`

- [ ] **SK-05-091** Every owned IR/request/result/error type that must cross the portable boundary has a lossless encoder.
- [ ] **SK-05-092** Schema encode after `describe()` preserves configured rank profiles and document summaries.
- [ ] **SK-05-093** Search/result encoding preserves the entire recursive result tree.
- [ ] **SK-05-094** Infinite relevance encodes into the explicit relevance union.
- [ ] **SK-05-095** Bytes/list/tensor/non-finite value variants preserve their original discriminators.
- [ ] **SK-05-096** Grouping continuations and metadata are preserved.
- [ ] **SK-05-097** Error codes/paths/details/retryability are preserved.
- [ ] **SK-05-098** API -> IR -> API round-trip tests exist for every represented semantic family.

## 5.5 Backend trait — `backend.rs`

- [ ] **SK-05-099** Backend receives only fully resolved owned IR.
- [ ] **SK-05-100** Backend API cannot observe unresolved wire omissions and reinterpret defaults.
- [ ] **SK-05-101** Backend method/result types are sufficient to preserve canonical result semantics.
- [ ] **SK-05-102** Backend errors can map to portable operation errors without string parsing.

## 5.6 Session/snapshot — `session.rs`, `snapshot.rs`

- [ ] **SK-05-103** Backend session lifecycle cannot use a closed/dropped backend.
- [ ] **SK-05-104** Closed-target behavior is deterministic and surfaced as a structured error.
- [ ] **SK-05-105** Snapshot/describe state is internally consistent under mutation/lifecycle operations.
- [ ] **SK-05-106** Snapshot data does not accidentally become a second source of semantics.
- [ ] **SK-05-107** `close()` waits for/inhibits in-flight operations according to the documented session contract and cannot deadlock with backend destruction.
- [ ] **SK-05-108** Session cloning/Arc ownership cannot extend a native backend past index-manifest/config lifetime assumptions.
- [ ] **SK-05-109** `describe()`/inspection snapshots cannot observe a mixture of old/new schema state during create/open/close transitions.

---
