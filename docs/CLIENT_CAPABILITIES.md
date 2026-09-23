# Client capability evidence

This file is generated from the canonical capability ledger,
reference/client-contracts/catalog.json, and SKC annotations colocated
with executable tests/examples. Do not edit this matrix by hand.

Positive client support is derived from evidence. Human-maintained metadata is
limited to blocked/not-applicable overrides in the catalog.

Legend: ergonomic = handwritten facade coverage; conformance = shared semantic
fixture coverage; E2E = real product/backend execution. For multi-scenario
capabilities, E2E 1/3 means one of three scenarios has E2E evidence.

| Capability | Area | Core / backend | Rust | Python | TypeScript | PHP |
| --- | --- | --- | --- | --- | --- | --- |
| Term and fielded contains queries (query.term) | query | — | — | — | — | ergonomic (http) |
| True and false query constants (query.boolean_constants) | query | — | — | — | — | — |
| Comparison queries (query.comparison) | query | — | — | — | — | — |
| Range queries (query.range) | query | — | — | — | — | — |
| Boolean query operators (query.boolean) | query | — | — | — | — | ergonomic (http) |
| Phrase queries (query.phrase) | query | — | ergonomic (native) | ergonomic (grpc) | — | — |
| Near and ordered-near proximity queries (query.proximity) | query | — | ergonomic (native) | ergonomic (grpc) | — | — |
| sameElement query (query.same_element) | query | — | — | ergonomic (grpc) | — | — |
| Fuzzy query (query.fuzzy) | query | — | ergonomic (native) | ergonomic (grpc) | — | — |
| Prefix, suffix, and substring queries (query.prefix_suffix_substring) | query | — | — | — | — | — |
| Alternatives queries (query.alternatives) | query | — | — | — | — | — |
| Nearest-neighbor vector search (query.nearest_neighbor) | query | — | — | — | ergonomic (node, shared) | ergonomic (http) |
| Geo-location and bounding-box queries (query.geo) | query | — | — | ergonomic (grpc) | — | — |
| Predicate queries (query.predicate) | query | — | — | — | — | — |
| WeakAnd and WAND queries (query.weighted_and_wand) | query | — | ergonomic (native) | ergonomic (grpc) | — | — |
| Weighted-set and dot-product queries (query.weighted_set_dot_product) | query | — | — | — | — | — |
| Equivalence queries (query.equivalence) | query | — | — | — | — | — |
| Rank query composition (query.rank) | query | — | — | — | — | — |
| userInput and userQuery text parsing (query.user_input_user_query) | query | — | — | — | — | — |
| Structured filters (filter.structured) | filter | — | — | — | — | — |
| Textual filter expressions (filter.textual) | filter | — | — | — | ergonomic (node, shared) | ergonomic (http) |
| Field sorting (search.sorting) | search | — | ergonomic (native) | — | ergonomic (node, shared) | ergonomic (http) |
| First/last missing-value sort placement (search.sort_missing_placement) | search | — | — | — | — | — |
| Limit and offset pagination (search.pagination) | search | — | ergonomic (native) | — | ergonomic (node, shared) | ergonomic (http) |
| Text search-field selection (search.search_fields) | search | — | — | — | — | — |
| Document-summary field projection (result.docsum_field_projection) | result | — | ergonomic (native) | ergonomic (grpc) | ergonomic (node, shared) | ergonomic (http) |
| Named summary-class selection (result.named_summary_class) | result | — | — | — | — | ergonomic (http) |
| Rich document-summary values (scalar subset) (result.rich_docsum_values) | result | — | — | — | — | — |
| Summary-feature readback (result.summary_features) | result | — | — | — | — | — |
| Highlighting (result.highlighting) | result | — | ergonomic (native) | ergonomic · conformance (grpc, shared) | conformance (shared) | ergonomic (http) |
| Dense tensor document readback (result.tensor_document_readback) | result | — | — | — | — | ergonomic (http) |
| Typed tensor feature readback (result.tensor_feature_readback) | result | — | — | — | — | — |
| Structured grouping and faceting (grouping.structured) | grouping | — | — | — | ergonomic (node, shared) | ergonomic (http) |
| Executable grouping expression/function subset (grouping.functions) | grouping | — | — | — | — | — |
| Grouping aggregations (grouping.aggregations) | grouping | — | — | — | ergonomic (node, shared) | ergonomic (http) |
| Summary aggregation (grouping.summary_aggregation) | grouping | — | — | — | — | ergonomic (http) |
| Grouping continuations (grouping.continuations) | grouping | — | — | — | — | — |
| Raw grouping source (grouping.raw_source) | grouping | — | — | — | — | — |
| Document insert, upsert, patch, delete, and get (update.document_crud) | update | — | ergonomic (native) | ergonomic (managed-local) | ergonomic (node) | ergonomic (http) |
| Arithmetic document updates (update.arithmetic) | update | — | — | — | — | ergonomic (http) |
| Create-if-nonexistent document updates (update.create_if_nonexistent) | update | — | — | — | — | ergonomic (http) |
| Conditional document mutations (update.conditional) | update | — | — | — | — | — |
| Collection add/remove updates (update.collection_add_remove) | update | — | — | — | — | — |
| Matched-element updates (update.matched_element) | update | — | — | — | — | — |
| Tensor modify updates (update.tensor_modify) | update | — | — | — | — | — |
| Dense tensor document values (tensor.dense_document_crud) | tensor | — | — | — | — | ergonomic (http) |
| YQL query source (query.yql) | query | — | — | — | — | ergonomic (http) |
| Analyzed text search with default WeakAnd semantics (query.text_weak_and) | query | — | — | — | — | ergonomic (http) |
| Typed analyzed-text query nodes (query.typed_text) | query | — | — | — | — | — |
| Native in-term queries (query.in_term) | query | — | — | — | — | — |
| NonEmpty query wrapper (query.non_empty) | query | — | — | — | — | — |
| Labeled query rank properties (query.labels) | query | — | — | — | — | — |
| Document-summary schema definitions (schema.document_summaries) | schema | — | — | — | — | — |
| Array-of-struct document fields (schema.array_struct_fields) | schema | — | — | — | — | — |
| Position fields and derived geo attributes (schema.position_geo_fields) | schema | — | — | — | — | — |
| Rank-profile selection (ranking.profile_selection) | ranking | — | — | — | — | ergonomic (http) |
| Numeric scalar query inputs (ranking.numeric_query_inputs) | ranking | — | — | — | — | ergonomic (http) |
| Dense tensor query inputs (ranking.tensor_query_inputs) | ranking | — | — | — | — | ergonomic (http) |
| Rank properties (ranking.rank_properties) | ranking | — | — | — | — | ergonomic (http) |
| Match-feature readback (result.match_features) | result | — | — | — | — | ergonomic (http) |
| Recursive grouping result trees (result.grouping_tree) | result | — | — | — | — | ergonomic (http) |
| Coverage and degradation metadata (result.degradation_metadata) | result | — | — | — | — | ergonomic (http) |
| Embedded single-node Vespa scope (platform.embedded_vespa_scope) | platform | — | — | — | — | — |

## Scenario evidence

### Term and fielded contains queries (query.term)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L750)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L751)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L415)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L416)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L417)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L752)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L128)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L418)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L753)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L40)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L89)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L754)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L217)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L41)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L90)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L755)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L218)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L42)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L91)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L756)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L219)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L43)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L92)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L757)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L758)
  - python / shared / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L222)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L220)
  - typescript / shared / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L273)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L759)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L760)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L129)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L761)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L130)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L762)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L763)
  - python / managed-local / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L39)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L764)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L765)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L766)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L767)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L768)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L769)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L770)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L771)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L772)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L773)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L774)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L775)

