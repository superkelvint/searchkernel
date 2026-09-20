# Vespa compatibility limitations

This file is generated from [`docs/capabilities.json`](docs/capabilities.json).
Search Kernel is a thin wrapper around the pinned Vespa engine, but its
public contract is intentionally smaller than Vespa's complete model.
Unsupported behavior must remain an explicit error; it must not be silently
approximated.

**Snapshot:** 2026-09-20

## Query

### sameElement query (`query.same_element`)

- Requires supported array-of-struct fields and native elementFilter; it is not a general nested struct or map model.

### Fuzzy query (`query.fuzzy`)

- The reduced userInput/userQuery grammar does not accept full Vespa fuzzy syntax; use the typed query path.

### Prefix, suffix, and substring queries (`query.prefix_suffix_substring`)

- These typed helpers are not part of the reduced textual userInput/userQuery grammar.

### Alternatives and equivalence queries (`query.alternatives`)

- alternatives currently accepts term children only; more general alternatives are rejected.

### Nearest-neighbor vector search (`query.nearest_neighbor`)

- The proven slice uses dense tensors and does not expose the full Vespa HNSW tuning surface.

### Geo-location and bounding-box queries (`query.geo`)

- Geo matching is delegated to Vespa and is not implemented in Rust; verified bounding-box edges are inclusive.

### Predicate queries (`query.predicate`)

- Only the supported signed-integer range representation and documented predicate expression shapes are lowered.

### Weighted-set, dot-product, weak-and, and WAND queries (`query.weighted_and_wand`)

- Only the currently supported typed annotations are exposed; unknown controls are rejected rather than dropped.

### userInput and userQuery text parsing (`query.user_input_user_query`)

- The parser intentionally supports a reduced grammar and rejects full Vespa escaping, boosts, range syntax, parser modes, and unsupported composites.

### YQL query source (`query.yql`)

- Only the existing low-level parser subset is supported; full Vespa YQL grammar, query profiles, and search chains are outside the wrapper.

### Analyzed text and WeakAnd queries (`query.text_weak_and`)

- text(...) is plain analyzed text with Vespa's default WeakAnd semantics; it is not a phrase.

### Native in-term queries (`query.in_term`)

- String and integer values use Vespa's native InTerm node rather than an OR of equality terms.

### NonEmpty query wrapper (`query.non_empty`)

- NonEmpty validates its lowered child and rejects statically empty boolean operators; it does not reproduce every parser-generated empty marker.

### Labeled query rank properties (`query.labels`)

- Labels are allocated by the adapter and exported as internal vespa.label.* rank properties; callers must not provide those properties.

## Filter

### Textual filter expressions (`filter.textual`)

- The filter expression language is a deliberate subset and is parsed and bound before entering canonical IR.

## Result

### Document summaries and stored-field projection (`result.summaries`)

- Vespa capability: grouped summary hits carry the selected docsum payload and remain beneath their owning group. Search Kernel limitation: the native seam currently forwards only grouped hit IDs and the adapter creates default/empty Hit values. User-visible impact: summary aggregation does not yet provide promised docsum fields and is not an executable public semantic capability. Reason/status: the native summary generator and typed grouped-hit payload are not implemented; blocked pending a faithful Vespa-backed result seam.

### Tensor feature and result readback (`result.tensor_readback`)

- Tensor decoding is a higher-layer capability and is not implied by the opaque Vespa feature container.

### Match-feature and summary-feature readback (`result.match_features`)

- Match features cross the Vespa boundary as doubles or borrowed opaque data; tensor decoding is tracked separately.

### Recursive grouping result trees (`result.grouping_tree`)

- The implemented plan exposes recursive groups, keys, aggregation values, and the canonical root, not Vespa's complete result-tree surface.

### Coverage, timeout, and issue metadata (`result.degradation_metadata`)

- Deterministic acceptance relies on real match-phase degradation; the frozen verifier does not fabricate a Vespa issue.

## Grouping

### Structured grouping and faceting (`grouping.structured`)

- Parallel grouping branches are not representable by the current linear native grouping plan.

### Grouping expressions and functions (`grouping.functions`)

- Vespa capability: grouping supports the full expression/function family accepted by the grouping parser. Search Kernel limitation: the embedded native adapter currently supports attributes, literals, arithmetic, aggregation references, and the verified function subset only; Member, MapLookup, and Bucket remain rejected. User-visible impact: valid portable requests using those forms fail with a structured unsupported error. Reason/status: the current linear ABI has no typed constructors for those Vespa expression nodes; arithmetic/literal/function lowering is implemented but needs dedicated real-engine acceptance coverage.

### Grouping aggregations (`grouping.aggregations`)

- The executable expression subset is documented by the native adapter; unsupported forms fail before execution.

### Summary aggregation (`grouping.summary_aggregation`)

- Vespa capability: summary() collects grouped hits using the surrounding max/default-hit policy and the requested summary class. Search Kernel limitation: finite max propagation is now retained where the linear plan has a grouping max, but grouped docsum payloads do not cross the native ABI. User-visible impact: callers cannot receive faithful summary fields from grouped Hit children. Reason/status: native summary generation and typed grouped-hit/docsum transfer remain unfinished; status is blocked, not a passing implementation.

### Grouping continuations (`grouping.continuations`)

- Vespa capability: grouping requests and responses carry opaque continuation tokens for paged group lists. Search Kernel limitation: tokens are rejected before native execution and are not represented in GroupingPlan or GroupingResult. User-visible impact: grouping pagination cannot be requested or resumed through V2. Reason/status: the current linear native ABI has no typed continuation input/output seam; implementation remains deferred.

### Raw grouping source (`grouping.raw_source`)

- Raw grouping text requires a parser and normalization step before it can enter canonical IR.

## Update

### Arithmetic document updates (`update.arithmetic`)

- Arithmetic update nodes are represented in the portable contract but are not yet proven through the complete native product path.

### Conditional and matched-element updates (`update.conditional`)

- The portable update model represents conditional forms, but the public ergonomic and native execution paths are not complete.

### Tensor modify updates (`update.tensor_modify`)

- Tensor modify operations are represented but not exposed as a complete ergonomic and verified product capability.

## Tensor

### Dense tensor document values (`tensor.dense_document_crud`)

- Sparse and mixed tensors remain unsupported in the current document and query-input paths.

## Schema

### Array-of-struct document fields (`schema.array_struct_fields`)

- Member queries are relative to the parent element and do not form a general nested struct or map model.

### Position fields and derived geo attributes (`schema.position_geo_fields`)

- The native adapter publishes Vespa-compatible derived zcurve data; Search Kernel does not implement geo matching independently.

## Platform

### Embedded single-node Vespa scope (`platform.embedded_vespa_scope`)

- Distributed routing, configuration services, schema deployment, federation, and other container-level Vespa facilities are outside this wrapper.

## Evidence and versioning

The state and limitations are cross-layer summaries. Behavioral authority
remains the frozen contract, pinned Vespa source, direct oracle, and
acceptance verifier in that order. Vespa behavior is pinned to the revision
recorded in [`reference/vespa/PINNED_VESPA.json`](reference/vespa/PINNED_VESPA.json).
