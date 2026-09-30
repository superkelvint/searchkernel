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
| Prefix, suffix, and substring queries (query.prefix_suffix_substring) | query | — | — | — | — | — | — |
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
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1409)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1047)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1047)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L840)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L840)

### True and false query constants (query.boolean_constants)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1410)

### Comparison queries (query.comparison)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1411)

### Range queries (query.range)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1412)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1413)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1054)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1054)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L847)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L847)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1414)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1048)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1048)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L841)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L489)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L841)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L415)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L50)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1415)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1049)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1049)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L842)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L490)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L842)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L416)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L51)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1416)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1051)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1051)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L843)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L491)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L843)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1417)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1050)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1050)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L844)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L492)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L844)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L417)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1418)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1055)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1055)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L848)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L848)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L49)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L165)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1419)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1052)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1052)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L845)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L493)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L845)

### Predicate queries (query.predicate)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1420)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1421)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1053)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1053)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L846)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L494)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L846)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L418)

### Weighted-set and dot-product queries (query.weighted_set_dot_product)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1422)

### Equivalence queries (query.equivalence)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1423)

### Rank query composition (query.rank)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1424)

### userInput and userQuery text parsing (query.user_input_user_query)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1425)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1430)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1056)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1056)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L849)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L849)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L89)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1431)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1057)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1057)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L850)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L850)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L217)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L90)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1433)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1059)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1059)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L851)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L851)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L218)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L47)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L91)

### Text search-field selection (search.search_fields)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1432)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1058)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1058)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1060)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1060)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L852)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L852)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L219)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L48)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L92)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1061)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1061)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L853)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L853)

### Summary-feature readback (result.summary_features)

- **Basic behavior** (basic)
  - ruby / http / conformance / conformance — [clients/ruby/test/run.rb](../clients/ruby/test/run.rb#L656)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1448)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1062)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1062)
  - python / shared / conformance / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L227)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L854)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L854)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L220)
  - typescript / shared / conformance / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L325)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1063)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1063)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L856)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L856)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1434)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1064)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1064)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L859)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L859)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L52)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L166)

### Executable grouping expression/function subset (grouping.functions)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1435)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1436)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1065)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1065)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L860)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L860)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L53)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L167)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1066)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1066)

### Grouping continuations (grouping.continuations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1437)

### Raw grouping source (grouping.raw_source)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1438)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1439)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1067)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1067)
  - python / managed-local / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - python / managed-local / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L861)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L861)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1440)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1068)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1068)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L862)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L862)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1441)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1069)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1069)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L863)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L863)

### Matched-element updates (update.matched_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1442)

### Tensor modify updates (update.tensor_modify)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1443)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1070)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1070)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L864)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L864)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1444)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1071)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1071)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L869)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L869)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1072)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1072)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)

### Typed analyzed-text query nodes (query.typed_text)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1426)

### Native in-term queries (query.in_term)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1427)

### NonEmpty query wrapper (query.non_empty)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1428)

### Labeled query rank properties (query.labels)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1429)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1445)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1073)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1073)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L865)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L865)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1074)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1074)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L866)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L866)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1446)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1075)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1075)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L868)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L868)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1447)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1076)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1076)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L867)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L867)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1077)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1077)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L857)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L857)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1449)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1078)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1078)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L858)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L858)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1450)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1079)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L1079)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L855)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L855)

