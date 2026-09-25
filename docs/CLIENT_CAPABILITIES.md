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
| Term and fielded contains queries (query.term) | query | — | — | — | — | ergonomic · conformance · E2E (http) |
| True and false query constants (query.boolean_constants) | query | — | — | — | — | conformance (http) |
| Comparison queries (query.comparison) | query | — | — | — | — | conformance (http) |
| Range queries (query.range) | query | — | — | — | — | conformance (http) |
| Boolean query operators (query.boolean) | query | — | — | — | — | ergonomic · conformance · E2E (http) |
| Phrase queries (query.phrase) | query | — | ergonomic (native) | ergonomic (grpc) | — | conformance (http) |
| Near and ordered-near proximity queries (query.proximity) | query | — | ergonomic (native) | ergonomic (grpc) | — | conformance (http) |
| sameElement query (query.same_element) | query | — | — | ergonomic (grpc) | — | conformance (http) |
| Fuzzy query (query.fuzzy) | query | — | ergonomic (native) | ergonomic (grpc) | — | conformance (http) |
| Prefix, suffix, and substring queries (query.prefix_suffix_substring) | query | — | — | — | — | — |
| Alternatives queries (query.alternatives) | query | — | — | — | — | — |
| Nearest-neighbor vector search (query.nearest_neighbor) | query | — | — | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| Geo-location and bounding-box queries (query.geo) | query | — | — | ergonomic (grpc) | — | conformance (http) |
| Predicate queries (query.predicate) | query | — | — | — | — | conformance (http) |
| WeakAnd and WAND queries (query.weighted_and_wand) | query | — | ergonomic (native) | ergonomic (grpc) | — | conformance (http) |
| Weighted-set and dot-product queries (query.weighted_set_dot_product) | query | — | — | — | — | conformance (http) |
| Equivalence queries (query.equivalence) | query | — | — | — | — | conformance (http) |
| Rank query composition (query.rank) | query | — | — | — | — | conformance (http) |
| userInput and userQuery text parsing (query.user_input_user_query) | query | — | — | — | — | conformance (http) |
| Structured filters (filter.structured) | filter | — | — | — | — | — |
| Textual filter expressions (filter.textual) | filter | — | — | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| Field sorting (search.sorting) | search | — | ergonomic (native) | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| First/last missing-value sort placement (search.sort_missing_placement) | search | — | — | — | — | — |
| Limit and offset pagination (search.pagination) | search | — | ergonomic (native) | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| Text search-field selection (search.search_fields) | search | — | — | — | — | ergonomic · conformance · E2E (http) |
| Document-summary field projection (result.docsum_field_projection) | result | — | ergonomic (native) | ergonomic (grpc) | ergonomic · E2E (node, shared) | ergonomic · E2E (http) |
| Named summary-class selection (result.named_summary_class) | result | — | — | — | — | ergonomic · E2E (http) |
| Rich document-summary values (scalar subset) (result.rich_docsum_values) | result | — | — | — | — | — |
| Summary-feature readback (result.summary_features) | result | — | — | — | — | — |
| Highlighting (result.highlighting) | result | — | ergonomic (native) | ergonomic · conformance (grpc, shared) | conformance (shared) | ergonomic · conformance · E2E 1/3 (http) |
| Dense tensor document readback (result.tensor_document_readback) | result | — | — | — | — | ergonomic · E2E (http) |
| Typed tensor feature readback (result.tensor_feature_readback) | result | — | — | — | — | — |
| Structured grouping and faceting (grouping.structured) | grouping | — | — | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| Executable grouping expression/function subset (grouping.functions) | grouping | — | — | — | — | conformance (http) |
| Grouping aggregations (grouping.aggregations) | grouping | — | — | — | ergonomic · E2E (node, shared) | ergonomic · conformance · E2E (http) |
| Summary aggregation (grouping.summary_aggregation) | grouping | — | — | — | — | ergonomic · E2E (http) |
| Grouping continuations (grouping.continuations) | grouping | — | — | — | — | conformance (http) |
| Raw grouping source (grouping.raw_source) | grouping | — | — | — | — | conformance (http) |
| Document insert, upsert, patch, delete, and get (update.document_crud) | update | — | ergonomic (native) | ergonomic (managed-local) | ergonomic · E2E (node) | ergonomic · conformance · E2E (http) |
| Arithmetic document updates (update.arithmetic) | update | — | — | — | — | ergonomic · conformance · E2E (http) |
| Create-if-nonexistent document updates (update.create_if_nonexistent) | update | — | — | — | — | ergonomic · conformance · E2E (http) |
| Conditional document mutations (update.conditional) | update | — | — | — | — | — |
| Collection add/remove updates (update.collection_add_remove) | update | — | — | — | — | — |
| Matched-element updates (update.matched_element) | update | — | — | — | — | conformance (http) |
| Tensor modify updates (update.tensor_modify) | update | — | — | — | — | conformance (http) |
| Dense tensor document values (tensor.dense_document_crud) | tensor | — | — | — | — | ergonomic · E2E (http) |
| YQL query source (query.yql) | query | — | — | — | — | ergonomic · conformance · E2E (http) |
| Analyzed text search with default WeakAnd semantics (query.text_weak_and) | query | — | — | — | — | ergonomic · E2E (http) |
| Typed analyzed-text query nodes (query.typed_text) | query | — | — | — | — | conformance (http) |
| Native in-term queries (query.in_term) | query | — | — | — | — | conformance (http) |
| NonEmpty query wrapper (query.non_empty) | query | — | — | — | — | conformance (http) |
| Labeled query rank properties (query.labels) | query | — | — | — | — | conformance (http) |
| Document-summary schema definitions (schema.document_summaries) | schema | — | — | — | — | — |
| Array-of-struct document fields (schema.array_struct_fields) | schema | — | — | — | — | — |
| Position fields and derived geo attributes (schema.position_geo_fields) | schema | — | — | — | — | — |
| Rank-profile selection (ranking.profile_selection) | ranking | — | — | — | — | ergonomic · conformance · E2E (http) |
| Numeric scalar query inputs (ranking.numeric_query_inputs) | ranking | — | — | — | — | ergonomic · E2E (http) |
| Dense tensor query inputs (ranking.tensor_query_inputs) | ranking | — | — | — | — | ergonomic · conformance · E2E (http) |
| Rank properties (ranking.rank_properties) | ranking | — | — | — | — | ergonomic · conformance · E2E (http) |
| Match-feature readback (result.match_features) | result | — | — | — | — | ergonomic · E2E (http) |
| Recursive grouping result trees (result.grouping_tree) | result | — | — | — | — | ergonomic · conformance · E2E (http) |
| Coverage and degradation metadata (result.degradation_metadata) | result | — | — | — | — | ergonomic · conformance · E2E (http) |
| Embedded single-node Vespa scope (platform.embedded_vespa_scope) | platform | — | — | — | — | — |

## Scenario evidence

### Term and fielded contains queries (query.term)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1240)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L750)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L750)

### True and false query constants (query.boolean_constants)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1241)

### Comparison queries (query.comparison)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1242)

### Range queries (query.range)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1243)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1244)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L751)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L751)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1245)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L415)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1246)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L416)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1247)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1248)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L417)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1249)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L752)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L752)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L128)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1250)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)

### Predicate queries (query.predicate)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1251)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1252)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L418)

### Weighted-set and dot-product queries (query.weighted_set_dot_product)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1253)

### Equivalence queries (query.equivalence)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1254)

### Rank query composition (query.rank)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1255)

### userInput and userQuery text parsing (query.user_input_user_query)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1256)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1261)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L753)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L753)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L40)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L40)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L89)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1262)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L754)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L754)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L217)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L41)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L41)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L90)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1264)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L756)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L756)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L218)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L42)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L42)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L91)

### Text search-field selection (search.search_fields)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1263)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L755)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L755)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L757)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L757)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L219)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L43)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L43)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L92)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L758)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L758)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1279)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L759)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L759)
  - python / shared / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L222)
  - python / grpc / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L220)
  - typescript / shared / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L302)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L760)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L760)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1265)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L761)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L761)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L129)

### Executable grouping expression/function subset (grouping.functions)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1266)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1267)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L762)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L762)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L130)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L763)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L763)

### Grouping continuations (grouping.continuations)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1268)

### Raw grouping source (grouping.raw_source)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1269)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1270)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L764)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L764)
  - python / managed-local / ergonomic — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - rust / native / ergonomic — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / e2e — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L39)
  - typescript / node / ergonomic — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L39)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1271)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L765)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L765)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1272)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L766)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L766)

### Matched-element updates (update.matched_element)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1273)

### Tensor modify updates (update.tensor_modify)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1274)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L767)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L767)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1275)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L768)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L768)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L769)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L769)

### Typed analyzed-text query nodes (query.typed_text)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1257)

### Native in-term queries (query.in_term)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1258)

### NonEmpty query wrapper (query.non_empty)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1259)

### Labeled query rank properties (query.labels)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1260)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1276)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L770)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L770)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L771)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L771)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1277)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L772)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L772)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1278)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L773)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L773)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L774)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L774)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1280)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L775)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L775)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1281)
  - php / http / e2e — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L776)
  - php / http / ergonomic — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L776)

