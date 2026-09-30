# Client capability evidence

This file is generated from the canonical capability ledger,
reference/client-contracts/catalog.json, and SKC annotations colocated
with executable tests/examples. Do not edit this matrix by hand.

Positive client support is derived from evidence. Human-maintained metadata is
limited to blocked/not-applicable overrides in the catalog.

Legend: ergonomic = handwritten facade coverage; conformance = shared semantic
fixture coverage; real-runtime = evidence owned by a registered E2E lane/runtime.
For multi-scenario capabilities, real-runtime 1/3 means one of three scenarios
has real-runtime evidence.

| Capability | Area | Core / backend | Rust | Python | TypeScript | PHP | Ruby |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Term and fielded contains queries (query.term) | query | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| True and false query constants (query.boolean_constants) | query | — | — | — | — | conformance (http) | — |
| Comparison queries (query.comparison) | query | — | — | — | — | conformance (http) | — |
| Range queries (query.range) | query | — | — | — | — | conformance (http) | — |
| Boolean query operators (query.boolean) | query | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Phrase queries (query.phrase) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | ergonomic · real-runtime (node) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Near and ordered-near proximity queries (query.proximity) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | ergonomic · real-runtime (node) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| sameElement query (query.same_element) | query | — | — | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Fuzzy query (query.fuzzy) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Prefix, suffix, and substring queries (query.prefix_suffix_substring) | query | — | ergonomic (native) | ergonomic (shared) | ergonomic (shared) | ergonomic (http) | ergonomic (http) |
| Alternatives queries (query.alternatives) | query | — | — | — | — | — | — |
| Nearest-neighbor vector search (query.nearest_neighbor) | query | — | — | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Geo-location and bounding-box queries (query.geo) | query | — | — | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Predicate queries (query.predicate) | query | — | — | — | — | conformance (http) | — |
| WeakAnd and WAND queries (query.weighted_and_wand) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Weighted-set and dot-product queries (query.weighted_set_dot_product) | query | — | — | — | — | conformance (http) | — |
| Equivalence queries (query.equivalence) | query | — | — | — | — | conformance (http) | — |
| Rank query composition (query.rank) | query | — | — | — | — | conformance (http) | — |
| userInput and userQuery text parsing (query.user_input_user_query) | query | — | — | — | — | conformance (http) | — |
| Structured filters (filter.structured) | filter | — | — | — | — | — | — |
| Textual filter expressions (filter.textual) | filter | — | — | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Field sorting (search.sorting) | search | — | ergonomic (native) | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| First/last missing-value sort placement (search.sort_missing_placement) | search | — | — | — | — | — | — |
| Limit and offset pagination (search.pagination) | search | — | ergonomic (native) | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Text search-field selection (search.search_fields) | search | — | — | — | — | ergonomic · conformance · real-runtime (http) | — |
| Document-summary field projection (result.docsum_field_projection) | result | — | ergonomic (native) | ergonomic · real-runtime (grpc) | ergonomic · real-runtime (node, shared) | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Named summary-class selection (result.named_summary_class) | result | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Rich document-summary values (scalar subset) (result.rich_docsum_values) | result | — | — | — | — | — | — |
| Summary-feature readback (result.summary_features) | result | — | — | — | — | — | conformance (http) |
| Highlighting (result.highlighting) | result | — | ergonomic (native) | ergonomic · conformance · real-runtime 1/3 (grpc, shared) | conformance (shared) | ergonomic · conformance · real-runtime 1/3 (http) | ergonomic · real-runtime 1/3 (managed-local) |
| Dense tensor document readback (result.tensor_document_readback) | result | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Typed tensor feature readback (result.tensor_feature_readback) | result | — | — | — | — | — | — |
| Structured grouping and faceting (grouping.structured) | grouping | — | — | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Executable grouping expression/function subset (grouping.functions) | grouping | — | — | — | — | conformance (http) | — |
| Grouping aggregations (grouping.aggregations) | grouping | — | — | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Summary aggregation (grouping.summary_aggregation) | grouping | — | — | — | — | ergonomic · real-runtime (http) | — |
| Grouping continuations (grouping.continuations) | grouping | — | — | — | — | conformance (http) | — |
| Raw grouping source (grouping.raw_source) | grouping | — | — | — | — | conformance (http) | — |
| Document insert, upsert, patch, delete, and get (update.document_crud) | update | — | ergonomic (native) | ergonomic · real-runtime (managed-local) | ergonomic · real-runtime (node) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Arithmetic document updates (update.arithmetic) | update | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Create-if-nonexistent document updates (update.create_if_nonexistent) | update | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Conditional document mutations (update.conditional) | update | — | — | — | — | — | — |
| Collection add/remove updates (update.collection_add_remove) | update | — | — | — | — | — | — |
| Matched-element updates (update.matched_element) | update | — | — | — | — | conformance (http) | — |
| Tensor modify updates (update.tensor_modify) | update | — | — | — | — | conformance (http) | — |
| Dense tensor document values (tensor.dense_document_crud) | tensor | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| YQL query source (query.yql) | query | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Analyzed text search with default WeakAnd semantics (query.text_weak_and) | query | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (http) |
| Typed analyzed-text query nodes (query.typed_text) | query | — | — | — | — | conformance (http) | — |
| Native in-term queries (query.in_term) | query | — | — | — | — | conformance (http) | — |
| NonEmpty query wrapper (query.non_empty) | query | — | — | — | — | conformance (http) | — |
| Labeled query rank properties (query.labels) | query | — | — | — | — | conformance (http) | — |
| Document-summary schema definitions (schema.document_summaries) | schema | — | — | — | — | — | — |
| Array-of-struct document fields (schema.array_struct_fields) | schema | — | — | — | — | — | — |
| Position fields and derived geo attributes (schema.position_geo_fields) | schema | — | — | — | — | — | — |
| Rank-profile selection (ranking.profile_selection) | ranking | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Numeric scalar query inputs (ranking.numeric_query_inputs) | ranking | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Dense tensor query inputs (ranking.tensor_query_inputs) | ranking | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Rank properties (ranking.rank_properties) | ranking | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Match-feature readback (result.match_features) | result | — | — | — | — | ergonomic · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Recursive grouping result trees (result.grouping_tree) | result | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Coverage and degradation metadata (result.degradation_metadata) | result | — | — | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Embedded single-node Vespa scope (platform.embedded_vespa_scope) | platform | — | — | — | — | — | — |
| Targeted multi-index daemon lifecycle protocol (daemon.multi_index_lifecycle) | daemon | — | — | — | — | — | — |

## Scenario evidence

### Term and fielded contains queries (query.term)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1422)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1188)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1188)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L950)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L950)

### True and false query constants (query.boolean_constants)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1423)

### Comparison queries (query.comparison)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1424)

### Range queries (query.range)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1425)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1426)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1195)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1195)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L957)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L957)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1427)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1189)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1189)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L951)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L501)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L951)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L415)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1428)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1190)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1190)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L952)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L502)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L952)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L416)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1429)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1192)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1192)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L953)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L503)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L953)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1430)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1191)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1191)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L954)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L504)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L954)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L417)

### Prefix, suffix, and substring queries (query.prefix_suffix_substring)

- **Basic behavior** (basic)
  - php / http / ergonomic / builder — [clients/php/tests/run.php](../clients/php/tests/run.php#L1271)
  - python / shared / ergonomic / builder — [clients/python/tests/test_flatbuffers_objects.py](../clients/python/tests/test_flatbuffers_objects.py#L734)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L276)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs](../core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs#L385)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L192)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1431)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1196)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1196)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L958)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L958)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L167)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1432)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1193)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1193)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L955)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L505)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L955)

### Predicate queries (query.predicate)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1433)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1434)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1194)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1194)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L956)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L506)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L956)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L418)

### Weighted-set and dot-product queries (query.weighted_set_dot_product)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1435)

### Equivalence queries (query.equivalence)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1436)

### Rank query composition (query.rank)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1437)

### userInput and userQuery text parsing (query.user_input_user_query)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1438)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1443)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1197)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1197)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L959)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L959)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L91)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1444)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1198)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1198)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L960)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L960)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L217)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L92)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1446)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1200)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1200)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L961)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L961)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L218)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L93)

### Text search-field selection (search.search_fields)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1445)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1199)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1199)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1201)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1201)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L962)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L962)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L219)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L94)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1202)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1202)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L963)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L963)

### Summary-feature readback (result.summary_features)

- **Basic behavior** (basic)
  - ruby / http / conformance / conformance — [clients/ruby/test/run.rb](../clients/ruby/test/run.rb#L656)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1461)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1203)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1203)
  - python / shared / conformance / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L227)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L964)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L964)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L220)
  - typescript / shared / conformance / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L325)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1204)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1204)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L966)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L966)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1447)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1205)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1205)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L969)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L969)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L168)

### Executable grouping expression/function subset (grouping.functions)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1448)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1449)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1206)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1206)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L970)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L970)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L54)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L54)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L169)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1207)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1207)

### Grouping continuations (grouping.continuations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1450)

### Raw grouping source (grouping.raw_source)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1451)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1452)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1208)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1208)
  - python / managed-local / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - python / managed-local / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L971)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L971)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1453)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1209)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1209)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L972)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L972)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1454)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1210)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1210)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L973)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L973)

### Matched-element updates (update.matched_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1455)

### Tensor modify updates (update.tensor_modify)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1456)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1211)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1211)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L974)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L974)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1457)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1212)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1212)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L979)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L979)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1213)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1213)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)

### Typed analyzed-text query nodes (query.typed_text)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1439)

### Native in-term queries (query.in_term)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1440)

### NonEmpty query wrapper (query.non_empty)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1441)

### Labeled query rank properties (query.labels)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1442)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1458)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1214)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1214)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L975)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L975)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1215)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1215)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L976)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L976)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1459)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1216)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1216)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L978)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L978)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1460)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1217)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1217)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L977)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L977)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1218)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1218)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L967)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L967)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1462)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1219)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1219)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L968)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L968)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1463)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1220)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1220)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L965)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L965)

