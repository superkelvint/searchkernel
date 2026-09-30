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
| Phrase queries (query.phrase) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
| Near and ordered-near proximity queries (query.proximity) | query | — | ergonomic (native) | ergonomic · real-runtime (grpc) | — | ergonomic · conformance · real-runtime (http) | ergonomic · real-runtime (http, managed-local) |
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
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1410)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L877)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L877)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L556)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L556)

### True and false query constants (query.boolean_constants)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1411)

### Comparison queries (query.comparison)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1412)

### Range queries (query.range)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1413)

### Boolean query operators (query.boolean)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1414)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L884)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L884)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L563)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L563)

### Phrase queries (query.phrase)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1415)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L878)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L878)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L370)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L557)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L489)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L557)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L415)

### Near and ordered-near proximity queries (query.proximity)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1416)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L879)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L879)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L371)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L558)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L490)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L558)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L416)

### sameElement query (query.same_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1417)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L881)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L881)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L400)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L559)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L491)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L559)

### Fuzzy query (query.fuzzy)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1418)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L880)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L880)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L372)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L560)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L492)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L560)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L417)

### Nearest-neighbor vector search (query.nearest_neighbor)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1419)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L885)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L885)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L564)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L564)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L44)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L165)

### Geo-location and bounding-box queries (query.geo)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1420)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L882)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L882)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L466)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L561)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L493)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L561)

### Predicate queries (query.predicate)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1421)

### WeakAnd and WAND queries (query.weighted_and_wand)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1422)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L883)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L883)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L373)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L562)
  - ruby / http / ergonomic / builder — [clients/ruby/test/facade.rb](../clients/ruby/test/facade.rb#L494)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L562)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L418)

### Weighted-set and dot-product queries (query.weighted_set_dot_product)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1423)

### Equivalence queries (query.equivalence)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1424)

### Rank query composition (query.rank)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1425)

### userInput and userQuery text parsing (query.user_input_user_query)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1426)

### Textual filter expressions (filter.textual)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1431)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L886)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L886)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L565)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L565)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L40)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L40)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L89)

### Field sorting (search.sorting)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1432)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L887)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L887)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L566)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L566)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L217)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L41)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L41)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L90)

### Limit and offset pagination (search.pagination)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1434)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L889)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L889)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L567)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L567)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L218)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L42)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L42)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L91)

### Text search-field selection (search.search_fields)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1433)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L888)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L888)

### Document-summary field projection (result.docsum_field_projection)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L890)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L890)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L507)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L568)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L568)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L219)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L43)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L43)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L92)

### Named summary-class selection (result.named_summary_class)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L891)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L891)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L569)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L569)

### Summary-feature readback (result.summary_features)

- **Basic behavior** (basic)
  - ruby / http / conformance / conformance — [clients/ruby/test/run.rb](../clients/ruby/test/run.rb#L656)

### Highlighting (result.highlighting)

- **Basic structured dynamic-teaser highlighting** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1449)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L892)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L892)
  - python / shared / conformance / conformance — [clients/python/tests/test_results.py](../clients/python/tests/test_results.py#L242)
  - python / grpc / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - python / grpc / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L508)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L570)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L570)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L220)
  - typescript / shared / conformance / conformance — [clients/typescript/tests/results.test.ts](../clients/typescript/tests/results.test.ts#L334)

### Dense tensor document readback (result.tensor_document_readback)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L893)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L893)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L572)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L572)

### Structured grouping and faceting (grouping.structured)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1435)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L894)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L894)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L575)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L575)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L45)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L166)

### Executable grouping expression/function subset (grouping.functions)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1436)

### Grouping aggregations (grouping.aggregations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1437)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L895)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L895)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L576)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L576)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L46)
  - typescript / shared / ergonomic / builder — [clients/typescript/tests/facade.test.ts](../clients/typescript/tests/facade.test.ts#L167)

### Summary aggregation (grouping.summary_aggregation)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L896)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L896)

### Grouping continuations (grouping.continuations)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1438)

### Raw grouping source (grouping.raw_source)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1439)

### Document insert, upsert, patch, delete, and get (update.document_crud)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1440)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L897)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L897)
  - python / managed-local / e2e / runtime — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - python / managed-local / ergonomic / builder — [clients/python/tests/test_grpc_e2e.py](../clients/python/tests/test_grpc_e2e.py#L257)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L577)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L239)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L577)
  - rust / native / ergonomic / builder — [core/crates/searchkernel/tests/native_e2e.rs](../core/crates/searchkernel/tests/native_e2e.rs#L14)
  - typescript / node / e2e / runtime — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L39)
  - typescript / node / ergonomic / builder — [clients/typescript/examples/try_searchkernel_client_api_e2e.ts](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts#L39)

### Arithmetic document updates (update.arithmetic)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1441)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L898)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L898)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L578)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L578)

### Create-if-nonexistent document updates (update.create_if_nonexistent)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1442)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L899)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L899)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L579)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L579)

### Matched-element updates (update.matched_element)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1443)

### Tensor modify updates (update.tensor_modify)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1444)

### Dense tensor document values (tensor.dense_document_crud)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L900)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L900)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L580)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L580)

### YQL query source (query.yql)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1445)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L901)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L901)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L585)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L585)

### Analyzed text search with default WeakAnd semantics (query.text_weak_and)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L902)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L902)
  - ruby / http / e2e / runtime — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)
  - ruby / http / ergonomic / builder — [clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb](../clients/ruby/examples/try_searchkernel_package_daemon_e2e.rb#L238)

### Typed analyzed-text query nodes (query.typed_text)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1427)

### Native in-term queries (query.in_term)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1428)

### NonEmpty query wrapper (query.non_empty)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1429)

### Labeled query rank properties (query.labels)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1430)

### Rank-profile selection (ranking.profile_selection)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1446)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L903)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L903)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L581)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L581)

### Numeric scalar query inputs (ranking.numeric_query_inputs)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L904)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L904)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L582)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L582)

### Dense tensor query inputs (ranking.tensor_query_inputs)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1447)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L905)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L905)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L584)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L584)

### Rank properties (ranking.rank_properties)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1448)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L906)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L906)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L583)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L583)

### Match-feature readback (result.match_features)

- **Basic behavior** (basic)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L907)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L907)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L573)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L573)

### Recursive grouping result trees (result.grouping_tree)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1450)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L908)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L908)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L574)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L574)

### Coverage and degradation metadata (result.degradation_metadata)

- **Basic behavior** (basic)
  - php / http / conformance / conformance — [clients/php/tests/run.php](../clients/php/tests/run.php#L1451)
  - php / http / e2e / runtime — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L909)
  - php / http / ergonomic / builder — [clients/php/examples/try_searchkernel_client_api_e2e.php](../clients/php/examples/try_searchkernel_client_api_e2e.php#L909)
  - ruby / managed-local / e2e / runtime — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L571)
  - ruby / managed-local / ergonomic / builder — [clients/ruby/examples/try_searchkernel_client_api_e2e.rb](../clients/ruby/examples/try_searchkernel_client_api_e2e.rb#L571)

