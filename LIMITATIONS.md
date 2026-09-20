# Vespa compatibility limitations

This file is generated from [`docs/capabilities.json`](docs/capabilities.json).
Search Kernel is a thin wrapper around the pinned Vespa engine, but its
public contract is intentionally smaller than Vespa's complete model.
Unsupported behavior must remain an explicit error; it must not be silently
approximated.

**Snapshot:** 2026-09-20

## Query

### Phrase queries (`query.phrase`)

- The Python high-level q namespace does not yet expose a phrase helper; low-level generated FlatBuffers can still represent QueryPhrase.

### Near and ordered-near proximity queries (`query.proximity`)

- QueryNear and QueryONear execute through the adapter, but neither high-level Rust Query nor the Python q facade exposes proximity constructors yet.

### sameElement query (`query.same_element`)

- Requires supported array-of-struct fields and native elementFilter; it is not a general nested struct or map model. The Rust query helper exists, but the current high-level Rust schema facade cannot itself define the required array-of-struct field.

### Fuzzy query (`query.fuzzy`)

- The reduced userInput/userQuery grammar does not accept full Vespa fuzzy syntax; use the typed query path. The Python high-level q facade does not yet expose fuzzy.

### Prefix, suffix, and substring queries (`query.prefix_suffix_substring`)

- The Rust facade lowers these conveniences through literal-safe matches patterns. They are not part of the reduced textual userInput/userQuery grammar, and the Python q facade does not yet expose them.

### Alternatives and equivalence queries (`query.alternatives`)

- Alternatives currently accepts term children only; more general alternatives are rejected. High-level Rust and Python facades do not expose these nodes.

### Nearest-neighbor vector search (`query.nearest_neighbor`)

- The proven public path uses dense tensor query inputs and exposes only target hits, approximate mode, distance threshold, and explore-additional-hits rather than the full Vespa HNSW tuning surface.

### Geo-location and bounding-box queries (`query.geo`)

- Real Vespa execution is verified, including inclusive bounding-box edges. The high-level schema facades do not yet expose position fields, so geo is not an end-to-end ergonomic SDK capability.

### Predicate queries (`query.predicate`)

- Only the supported predicate feature/range representation and documented expression shapes are lowered. Predicate field definitions are not exposed by the high-level Rust or Python schema facades.

### WeakAnd, weighted-set, dot-product, and WAND queries (`query.weighted_and_wand`)

- Only the currently supported typed annotations are exposed; unknown controls are rejected rather than dropped. The Python q facade does not yet expose these weighted query constructors.

### userInput and userQuery text parsing (`query.user_input_user_query`)

- The parser intentionally supports a reduced grammar and rejects full Vespa escaping, boosts, range syntax, parser modes, and unsupported composites. Neither high-level facade currently exposes userInput/userQuery constructors.

### YQL query source (`query.yql`)

- Only the existing low-level parser subset is supported; full Vespa YQL grammar, query profiles, and search chains are outside the embedded wrapper. The public Rust Index has search()/browse() but no yql() entry point.

### Analyzed text search with default WeakAnd semantics (`query.text_weak_and`)

- Primary search(text) is analyzed text retrieval with Vespa WeakAnd-style default matching; it is not a phrase query. The low-level QueryText node is broader than the current facade helpers.

### Native in-term queries (`query.in_term`)

- String and integer values use Vespa's native InTerm node rather than an OR of equality terms. The Python q facade does not yet expose in().

### NonEmpty query wrapper (`query.non_empty`)

- NonEmpty validates its lowered child and rejects statically empty boolean operators; it does not reproduce every parser-generated empty marker. No high-level facade helper exists yet.

### Labeled query rank properties (`query.labels`)

- Labels are allocated by the adapter and exported as internal vespa.label.* rank properties; callers must not provide those properties. No high-level labeled-query helper exists yet.

## Filter

### Structured filters (`filter.structured`)

- The Python high-level facade currently uses textual filters; it does not expose a typed structured-filter helper namespace.

### Textual filter expressions (`filter.textual`)

- The filter expression language is a deliberate subset and is parsed and bound before entering canonical IR. The Rust SearchBuilder currently accepts typed Filter values only; it has no public textual-filter overload.

## Search

### Field sorting (`search.sorting`)

- The native hit ABI supports executing field sorts but does not expose Vespa sort values, so public Hit.sort_values is not populated by the real backend.

### First/last missing-value sort placement (`search.sort_missing_placement`)

- Rust exposes missing-first and missing-last. The portable/native path also has a missing-as-value form, but it is not exposed by the Rust convenience type. Python's high-level sort() currently has no missing-value option.

### Limit and offset pagination (`search.pagination`)

- Python additionally exposes page() as a convenience. Rust currently exposes limit/offset but no page() helper.

### Text search-field selection (`search.search_fields`)

- Field selection executes, but non-default per-field weights are rejected by the current native adapter. The Python high-level SearchBuilder does not yet expose search_field/search_fields.

## Result

### Document-summary field projection (`result.docsum_field_projection`)

- Stored scalar fields are projected through the current native summary seam. Named summary-class selection, rich docsum value decoding, and summary-feature readback are tracked separately.

### Named summary-class selection (`result.named_summary_class`)

- Presentation.summary survives canonical semantics, but validate_search_request rejects it with presentation_not_supported. Neither high-level facade currently exposes summary-class selection.

### Rich document-summary values (scalar subset) (`result.rich_docsum_values`)

- Only scalar string, number, and boolean docsum values are executable through the current native result seam. Arrays, structs/maps, tensors, and other rich Vespa docsum values are not decoded faithfully.

### Summary-feature readback (`result.summary_features`)

- The portable Hit schema and IR represent summary features, but the current adapter leaves summary_features empty instead of decoding native summary-feature values.

### Highlighting (`result.highlighting`)

- The Rust SearchBuilder exposes highlight(), but the real Vespa-backed path deliberately returns highlighting_native_result_unavailable because generated fragments/spans do not cross the native result seam. Python has no high-level highlight method yet.

### Dense tensor document readback (`result.tensor_document_readback`)

- Dense tensor fields round-trip through document GET/projection and reopen. Sparse/mixed document tensors are outside the supported high-level path; Python currently exposes readback as the tagged $searchkernel_tensor object rather than reconstructing a Tensor instance.

### Typed tensor feature readback (`result.tensor_feature_readback`)

- Tensor-backed match features currently cross the native result seam as opaque data bytes. The adapter does not decode them into a portable TensorValue.

### Match-feature readback (`result.match_features`)

- Scalar match features are decoded. Tensor-backed feature output remains opaque bytes; summary-feature readback is tracked separately. Rank features remain unpopulated.

### Recursive grouping result trees (`result.grouping_tree`)

- The canonical IR/result protocol is a heterogeneous recursive ResultNode tree. Python exposes root/children losslessly. Rust currently projects recursive ResultGroup values into groups/aggregations and does not expose the canonical mixed root; nested Hit children would be dropped by that Rust projection.

### Coverage and degradation metadata (`result.degradation_metadata`)

- The real backend populates coverage counters and match-phase/timeout/ANN degradation flags. Portable timing, trace, arbitrary metadata, and native Vespa UniqueIssues are not currently populated by this backend; UniqueIssues are deliberately not fabricated into SearchResult.errors.

## Grouping

### Structured grouping and faceting (`grouping.structured`)

- The native lowering is a linear grouping plan and cannot execute arbitrary parallel grouping branches from the full portable tree.

### Executable grouping expression/function subset (`grouping.functions`)

- The native adapter supports attributes, literals, arithmetic, aggregation references, aliases, and the verified function subset. Member, MapLookup, and Bucket remain rejected. Rust's public GroupingExpression currently exposes attributes only; Python exposes the richer expression facade.

### Grouping aggregations (`grouping.aggregations`)

- The native layer supports count, average, sum, min, max, stddev, xor, and quantiles. Rust's high-level GroupingAggregation exposes count/average/sum/min/max; Python additionally exposes stddev. Summary aggregation is tracked separately because its hit payload is not complete.

### Summary aggregation (`grouping.summary_aggregation`)

- The native grouping skeleton preserves summary() and finite max propagation, and the Python facade can construct agg.summary(), but grouped docsum payloads do not cross the native ABI. The Python E2E keeps this case explicitly blocked. Rust's current GroupingAggregation facade has no summary variant.

### Grouping continuations (`grouping.continuations`)

- Continuation tokens are preserved in GroupingRequestIr and in the portable result tree, but native grouping lowering rejects any request with continuations. Python result objects preserve tokens if received, yet there is no production native round-trip or high-level continuation request helper.

### Raw grouping source (`grouping.raw_source`)

- Raw grouping text is represented as caller intent, but resolve_grouping rejects it until a parser/normalizer can produce canonical structured grouping IR.

## Update

### Document insert, upsert, patch, delete, and get (`update.document_crud`)

- This row covers the ordinary unconditional operations and Patch set/unset. Conditional mutations and advanced update-operation families are tracked separately. Ordered multiple updates targeting the same field are represented by Vespa's model but currently return Unsupported because the portable IR does not yet preserve that ordered field-update sequence.

### Arithmetic document updates (`update.arithmetic`)

- Increment, decrement, multiply, and divide execute through canonical FlatBuffers -> IR -> native DocumentUpdate in Python. The high-level Rust product API currently has Patch set/unset only and no advanced UpdateBuilder.

### Create-if-nonexistent document updates (`update.create_if_nonexistent`)

- Python UpdateBuilder.create() is proven through the real native path. The high-level Rust product API does not yet expose advanced document updates.

### Conditional document mutations (`update.conditional`)

- Conditions are preserved in canonical IR for upsert/delete/update, but the Vespa adapter returns conditional_mutation_not_supported. Python exposes condition arguments/builders; Rust does not.

### Collection add/remove updates (`update.collection_add_remove`)

- AddUpdate and RemoveUpdate are represented in FlatBuffers, but update_to_ir currently rejects them as update_operation_not_supported before canonical IR.

### Matched-element updates (`update.matched_element`)

- MatchUpdate is represented in FlatBuffers, but the current UpdateOperationIr has no matched-element variant and update_to_ir rejects it before canonical IR.

### Tensor modify updates (`update.tensor_modify`)

- TensorModifyUpdate is represented in FlatBuffers, but there is no tensor-modify UpdateOperationIr and update_to_ir rejects it before canonical IR.

## Tensor

### Dense tensor document values (`tensor.dense_document_crud`)

- Sparse and mixed tensors remain unsupported in the current high-level document and query-input paths.

## Schema

### Document-summary schema definitions (`schema.document_summaries`)

- Document-summary definitions and fields survive FlatBuffers-to-IR semantic conversion, but native schema generation does not install named document summaries for execution.

### Array-of-struct document fields (`schema.array_struct_fields`)

- The portable/native path supports the existing array<object> shape used by SameElement. The high-level Rust Field and Python field-spec facades expose only primitive/tensor kinds today; nested structs, maps, and heterogeneous object members remain unsupported.

### Position fields and derived geo attributes (`schema.position_geo_fields`)

- The native adapter publishes Vespa-compatible derived zcurve data and real geo queries are verified. Position is not exposed by the high-level Rust or Python schema builders.

## Ranking

### Rank-profile selection (`ranking.profile_selection`)

- The profile must exist in the compiled index schema; Search Kernel does not introduce a second ranking language.

### Numeric scalar query inputs (`ranking.numeric_query_inputs`)

- The public Rust QueryInputValue currently exposes integer, unsigned integer, float, and tensor values; the proven scalar real-engine path is numeric. Expression query inputs are not exposed by the high-level facades.

### Dense tensor query inputs (`ranking.tensor_query_inputs`)

- Only literal dense tensors are executable by the current native query-input seam; sparse/mixed tensors and expression inputs are rejected.

### Rank properties (`ranking.rank_properties`)

- Adapter-owned vespa.label.* properties are reserved for labeled queries and cannot be supplied by callers.

## Platform

### Embedded single-node Vespa scope (`platform.embedded_vespa_scope`)

- Distributed routing, configuration services, schema deployment, federation, and other container-level Vespa facilities are outside this wrapper.

## Evidence and versioning

The state and limitations are cross-layer summaries. Behavioral authority
remains the frozen contract, pinned Vespa source, direct oracle, and
acceptance verifier in that order. Vespa behavior is pinned to the revision
recorded in [`reference/vespa/PINNED_VESPA.json`](reference/vespa/PINNED_VESPA.json).
