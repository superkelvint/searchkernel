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
| Structured grouping and faceting (grouping.structured) | grouping | — | ergonomic (native) | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Executable grouping expression/function subset (grouping.functions) | grouping | — | ergonomic (native) | — | — | conformance (http) | — |
| Grouping aggregations (grouping.aggregations) | grouping | — | ergonomic (native) | — | ergonomic · real-runtime (node, shared) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Summary aggregation (grouping.summary_aggregation) | grouping | — | — | — | — | ergonomic · real-runtime (http) | — |
| Grouping continuations (grouping.continuations) | grouping | — | — | — | — | conformance (http) | — |
| Raw grouping source (grouping.raw_source) | grouping | — | — | — | — | conformance (http) | — |
| Document insert, upsert, patch, delete, and get (update.document_crud) | update | — | ergonomic (native) | ergonomic · real-runtime (managed-local) | ergonomic · real-runtime (node) | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Arithmetic document updates (update.arithmetic) | update | — | ergonomic · real-runtime (native) | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
| Create-if-nonexistent document updates (update.create_if_nonexistent) | update | — | ergonomic · real-runtime (native) | — | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (managed-local) |
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
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1480)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1227)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1227)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L953)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L953)

### True and false query constants (query.boolean_constants)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1481)

### Comparison queries (query.comparison)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1482)

### Range queries (query.range)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1483)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1484)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1234)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1234)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L960)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L960)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1485)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1228)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1228)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L505)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L505)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L954)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L565)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L954)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L506)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1486)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1229)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1229)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L506)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L506)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L955)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L566)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L955)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L507)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1487)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1231)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1231)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L535)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L535)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L956)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L567)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L956)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1488)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1230)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1230)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L957)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L568)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L957)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L508)

### Prefix, suffix, and substring queries (query.prefix_suffix_substring)

- **Basic behavior** (basic)
  - php / http / ergonomic / builder — [clients/php/tests/run.php](../clients/php/tests/run.php#L1329)
  - python / shared / ergonomic / builder — [clients/python/tests/test_flatbuffers_objects.py](../clients/python/tests/test_flatbuffers_objects.py#L770)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L288)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs](../core/crates/searchkernel/examples/try_searchkernel_client_api_e2e.rs#L385)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L204)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1489)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1235)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1235)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L961)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L961)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L179)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1490)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1232)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1232)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L600)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L600)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L958)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L569)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L958)

### Predicate queries (query.predicate)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1491)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1492)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1233)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1233)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L959)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L570)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L959)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L509)

### Weighted-set and dot-product queries (query.weighted_set_dot_product)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1493)

### Equivalence queries (query.equivalence)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1494)

### Rank query composition (query.rank)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1495)

### userInput and userQuery text parsing (query.user_input_user_query)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1496)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1501)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1236)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1236)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L962)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L962)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L103)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1502)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1237)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1237)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L963)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L963)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L308)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L104)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1504)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1239)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1239)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L964)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L964)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L309)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L105)

### Text search-field selection (search.search_fields)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1503)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1238)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1238)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1240)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1240)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L640)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L640)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L965)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L965)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L310)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L106)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1241)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1241)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L966)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L966)

### Summary-feature readback (result.summary_features)

- **Basic behavior** (basic)
  - ruby / http / conformance / conformance — [clients/ruby/test/run.rb](../clients/ruby/test/run.rb#L682)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1519)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1242)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1242)
  - python / shared / conformance / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L242)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L641)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L641)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L967)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L967)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L311)
  - typescript / shared / conformance / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L334)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1243)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1243)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L969)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L969)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1505)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1244)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1244)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L972)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L972)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/grouping_native_e2e.rs](../core/crates/searchkernel/tests/grouping_native_e2e.rs#L9)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L180)

### Executable grouping expression/function subset (grouping.functions)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1506)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/grouping_native_e2e.rs](../core/crates/searchkernel/tests/grouping_native_e2e.rs#L10)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1507)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1245)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1245)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L973)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L973)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/grouping_native_e2e.rs](../core/crates/searchkernel/tests/grouping_native_e2e.rs#L11)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L54)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L54)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L181)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1246)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1246)

### Grouping continuations (grouping.continuations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1508)

### Raw grouping source (grouping.raw_source)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1509)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1510)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1247)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1247)
  - python / managed-local / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L301)
  - python / managed-local / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L301)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L271)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L974)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L271)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L974)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1511)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1248)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1248)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L975)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L975)
  - rust / native / e2e / runtime — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L174)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L174)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1512)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1249)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1249)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L976)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L976)
  - rust / native / e2e / runtime — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L175)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L175)

### Matched-element updates (update.matched_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1513)

### Tensor modify updates (update.tensor_modify)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1514)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1250)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1250)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L977)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L977)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1515)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1251)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1251)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L982)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L982)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1252)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1252)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L270)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L270)

### Typed analyzed-text query nodes (query.typed_text)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1497)

### Native in-term queries (query.in_term)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1498)

### NonEmpty query wrapper (query.non_empty)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1499)

### Labeled query rank properties (query.labels)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1500)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1516)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1253)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1253)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L978)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L978)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1254)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1254)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L979)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L979)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1517)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1255)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1255)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L981)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L981)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1518)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1256)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1256)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L980)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L980)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1257)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1257)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L970)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L970)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1520)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1258)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1258)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L971)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L971)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1521)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1259)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1259)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L968)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L968)

