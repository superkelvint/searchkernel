# Capability ledger

This file is generated from [`capabilities.json`](capabilities.json).
It is the current cross-layer status view; contracts and verifiers remain
the authority for required behavior.

**Snapshot:** 2026-09-20

The state columns mean: represented in the portable contract, resolved by
canonical semantics, executable through the backend, and ergonomic in the
named language facade.

| Capability | Area | Represented | Semantic | Executable | Rust | Python | Evidence |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Boolean query operators (`query.boolean`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m7/verify-m7.sh](../reference/vespa/m7/verify-m7.sh) |
| Phrase and proximity queries (`query.phrase_near`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m7/verify-m7.sh](../reference/vespa/m7/verify-m7.sh) |
| sameElement query (`query.same_element`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/QUERY_SOURCE_TRACE.md](../reference/vespa/QUERY_SOURCE_TRACE.md), [docs/architecture.md](../docs/architecture.md) |
| Fuzzy query (`query.fuzzy`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m7/CONTRACT.md](../reference/vespa/m7/CONTRACT.md), [docs/architecture.md](../docs/architecture.md) |
| Prefix, suffix, and substring queries (`query.prefix_suffix_substring`) | query | yes | yes | yes | yes | no | [core/crates/searchkernel/src/query.rs](../core/crates/searchkernel/src/query.rs), [reference/vespa/m7/CONTRACT.md](../reference/vespa/m7/CONTRACT.md), [docs/architecture.md](../docs/architecture.md) |
| Alternatives and equivalence queries (`query.alternatives`) | query | yes | yes | yes | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Nearest-neighbor vector search (`query.nearest_neighbor`) | query | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m12/verify-m12.sh](../reference/vespa/m12/verify-m12.sh), [clients/python/tests/test_flatbuffers_objects.py](../clients/python/tests/test_flatbuffers_objects.py) |
| Geo-location and bounding-box queries (`query.geo`) | query | yes | yes | yes | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Predicate queries (`query.predicate`) | query | yes | yes | yes | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Weighted-set, dot-product, weak-and, and WAND queries (`query.weighted_and_wand`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [core/crates/searchkernel/src/query.rs](../core/crates/searchkernel/src/query.rs), [docs/architecture.md](../docs/architecture.md) |
| userInput and userQuery text parsing (`query.user_input_user_query`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Structured filters (`filter.structured`) | filter | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m7/verify-m7.sh](../reference/vespa/m7/verify-m7.sh), [core/crates/searchkernel/src/filter.rs](../core/crates/searchkernel/src/filter.rs) |
| Textual filter expressions (`filter.textual`) | filter | yes | yes | yes | yes | yes | [docs/filter-expression-language.md](../docs/filter-expression-language.md), [core/crates/searchkernel-ir/src/filter_expression.rs](../core/crates/searchkernel-ir/src/filter_expression.rs) |
| Sorting and missing-value placement (`search.sorting`) | search | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m8/verify-m8.sh](../reference/vespa/m8/verify-m8.sh), [core/crates/searchkernel/src/search.rs](../core/crates/searchkernel/src/search.rs) |
| Limit and offset pagination (`search.pagination`) | search | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m8/verify-m8.sh](../reference/vespa/m8/verify-m8.sh) |
| Document summaries and stored-field projection (`result.summaries`) | result | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m8/verify-m8.sh](../reference/vespa/m8/verify-m8.sh), [docs/architecture.md](../docs/architecture.md) |
| Highlighting (`result.highlighting`) | result | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/client-api-spec.md](../docs/client-api-spec.md) |
| Tensor feature and result readback (`result.tensor_readback`) | result | yes | yes | no | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Structured grouping and faceting (`grouping.structured`) | grouping | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m9/verify-m9.sh](../reference/vespa/m9/verify-m9.sh), [reference/vespa/m13/grouping-yql/verify-grouping-yql.sh](../reference/vespa/m13/grouping-yql/verify-grouping-yql.sh) |
| Grouping expressions and functions (`grouping.functions`) | grouping | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Grouping aggregations (`grouping.aggregations`) | grouping | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m9/verify-m9.sh](../reference/vespa/m9/verify-m9.sh), [docs/architecture.md](../docs/architecture.md) |
| Summary aggregation (`grouping.summary_aggregation`) | grouping | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Grouping continuations (`grouping.continuations`) | grouping | yes | no | no | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Raw grouping source (`grouping.raw_source`) | grouping | yes | no | no | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Document insert, upsert, patch, delete, and get (`update.document_crud`) | update | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/i/verify-i.sh](../reference/vespa/i/verify-i.sh), [docs/status.md](../docs/status.md) |
| Arithmetic document updates (`update.arithmetic`) | update | yes | yes | no | no | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Conditional and matched-element updates (`update.conditional`) | update | yes | yes | no | no | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Tensor modify updates (`update.tensor_modify`) | update | yes | yes | no | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Dense tensor document values (`tensor.dense_document_crud`) | tensor | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m13/verify-m13.sh](../reference/vespa/m13/verify-m13.sh), [core/crates/searchkernel/src/tensor.rs](../core/crates/searchkernel/src/tensor.rs) |
| YQL query source (`query.yql`) | query | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m13/grouping-yql/verify-grouping-yql.sh](../reference/vespa/m13/grouping-yql/verify-grouping-yql.sh), [docs/architecture.md](../docs/architecture.md) |
| Analyzed text and WeakAnd queries (`query.text_weak_and`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m7/verify-m7.sh](../reference/vespa/m7/verify-m7.sh), [docs/architecture.md](../docs/architecture.md) |
| Native in-term queries (`query.in_term`) | query | yes | yes | yes | yes | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| NonEmpty query wrapper (`query.non_empty`) | query | yes | yes | yes | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Labeled query rank properties (`query.labels`) | query | yes | yes | yes | no | no | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Array-of-struct document fields (`schema.array_struct_fields`) | schema | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Position fields and derived geo attributes (`schema.position_geo_fields`) | schema | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [docs/architecture.md](../docs/architecture.md) |
| Match-feature and summary-feature readback (`result.match_features`) | result | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/searchreply/CONTRACT.md](../reference/vespa/searchreply/CONTRACT.md) |
| Recursive grouping result trees (`result.grouping_tree`) | result | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/m9/verify-m9.sh](../reference/vespa/m9/verify-m9.sh) |
| Coverage, timeout, and issue metadata (`result.degradation_metadata`) | result | yes | yes | yes | yes | yes | [api/searchkernel.fbs](../api/searchkernel.fbs), [reference/vespa/searchreply/CONTRACT.md](../reference/vespa/searchreply/CONTRACT.md) |
| Embedded single-node Vespa scope (`platform.embedded_vespa_scope`) | platform | no | no | no | no | no | [docs/architecture.md](../docs/architecture.md), [docs/status.md](../docs/status.md) |

## Limitations at a glance

Capabilities with known constraints are collected in the generated
[`LIMITATIONS.md`](../LIMITATIONS.md).

- `query.same_element` — Requires supported array-of-struct fields and native elementFilter; it is not a general nested struct or map model.
- `query.fuzzy` — The reduced userInput/userQuery grammar does not accept full Vespa fuzzy syntax; use the typed query path.
- `query.prefix_suffix_substring` — These typed helpers are not part of the reduced textual userInput/userQuery grammar.
- `query.alternatives` — alternatives currently accepts term children only; more general alternatives are rejected.
- `query.nearest_neighbor` — The proven slice uses dense tensors and does not expose the full Vespa HNSW tuning surface.
- `query.geo` — Geo matching is delegated to Vespa and is not implemented in Rust; verified bounding-box edges are inclusive.
- `query.predicate` — Only the supported signed-integer range representation and documented predicate expression shapes are lowered.
- `query.weighted_and_wand` — Only the currently supported typed annotations are exposed; unknown controls are rejected rather than dropped.
- `query.user_input_user_query` — The parser intentionally supports a reduced grammar and rejects full Vespa escaping, boosts, range syntax, parser modes, and unsupported composites.
- `filter.textual` — The filter expression language is a deliberate subset and is parsed and bound before entering canonical IR.
- `result.summaries` — Grouped-hit/docsum ownership and external document-id fidelity are not proven for every summary class.
- `result.tensor_readback` — Tensor decoding is a higher-layer capability and is not implied by the opaque Vespa feature container.
- `grouping.structured` — Parallel grouping branches are not representable by the current linear native grouping plan.
- `grouping.functions` — Member, MapLookup, Bucket, and some validated Vespa function names still require dedicated native constructors.
- `grouping.aggregations` — The executable expression subset is documented by the native adapter; unsupported forms fail before execution.
- `grouping.summary_aggregation` — Grouped summary result fidelity remains a known real-Vespa verification gap for some summary classes.
- `grouping.continuations` — Continuation tokens are not currently represented end-to-end in the linear grouping plan, native ABI, and portable result.
- `grouping.raw_source` — Raw grouping text requires a parser and normalization step before it can enter canonical IR.
- `update.arithmetic` — Arithmetic update nodes are represented in the portable contract but are not yet proven through the complete native product path.
- `update.conditional` — The portable update model represents conditional forms, but the public ergonomic and native execution paths are not complete.
- `update.tensor_modify` — Tensor modify operations are represented but not exposed as a complete ergonomic and verified product capability.
- `tensor.dense_document_crud` — Sparse and mixed tensors remain unsupported in the current document and query-input paths.
- `query.yql` — Only the existing low-level parser subset is supported; full Vespa YQL grammar, query profiles, and search chains are outside the wrapper.
- `query.text_weak_and` — text(...) is plain analyzed text with Vespa's default WeakAnd semantics; it is not a phrase.
- `query.in_term` — String and integer values use Vespa's native InTerm node rather than an OR of equality terms.
- `query.non_empty` — NonEmpty validates its lowered child and rejects statically empty boolean operators; it does not reproduce every parser-generated empty marker.
- `query.labels` — Labels are allocated by the adapter and exported as internal vespa.label.* rank properties; callers must not provide those properties.
- `schema.array_struct_fields` — Member queries are relative to the parent element and do not form a general nested struct or map model.
- `schema.position_geo_fields` — The native adapter publishes Vespa-compatible derived zcurve data; Search Kernel does not implement geo matching independently.
- `result.match_features` — Match features cross the Vespa boundary as doubles or borrowed opaque data; tensor decoding is tracked separately.
- `result.grouping_tree` — The implemented plan exposes recursive groups, keys, aggregation values, and the canonical root, not Vespa's complete result-tree surface.
- `result.degradation_metadata` — Deterministic acceptance relies on real match-phase degradation; the frozen verifier does not fabricate a Vespa issue.
- `platform.embedded_vespa_scope` — Distributed routing, configuration services, schema deployment, federation, and other container-level Vespa facilities are outside this wrapper.
