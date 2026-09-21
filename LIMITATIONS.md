# Vespa compatibility limitations

This file is generated from [`docs/capabilities.json`](docs/capabilities.json).
Search Kernel is a thin wrapper around the pinned Vespa engine, but its
public contract is intentionally smaller than Vespa's complete model.
Unsupported behavior must remain an explicit error; it must not be silently
approximated.

**Snapshot:** 2026-09-20

## Query

### Phrase queries (`query.phrase`)

- The Python and TypeScript high-level q namespaces do not yet expose

### Near and ordered-near proximity queries (`query.proximity`)

- the high-level Rust Query, Python q, or TypeScript q facades expose proximity constructors yet

### sameElement query (`query.same_element`)

- Requires supported array-of-struct fields and native elementFilter; it is not a general nested struct or map model. The Rust query helper exists, but the current high-level Rust schema facade cannot itself define the required array-of-struct field.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Fuzzy query (`query.fuzzy`)

- The Python and TypeScript high-level q facades do not yet expose

### Prefix, suffix, and substring queries (`query.prefix_suffix_substring`)

- the Python and TypeScript q facades do not yet expose

### Alternatives and equivalence queries (`query.alternatives`)

- Alternatives currently accepts term children only; more general alternatives are rejected. High-level Rust and Python facades do not expose these nodes.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Nearest-neighbor vector search (`query.nearest_neighbor`)

- The proven public path uses dense tensor query inputs and exposes only target hits, approximate mode, distance threshold, and explore-additional-hits rather than the full Vespa HNSW tuning surface.

### Geo-location and bounding-box queries (`query.geo`)

- Real Vespa execution is verified, including inclusive bounding-box edges. The high-level schema facades do not yet expose position fields, so geo is not an end-to-end ergonomic SDK capability.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Predicate queries (`query.predicate`)

- Only the supported predicate feature/range representation and documented expression shapes are lowered. Predicate field definitions are not exposed by the high-level Rust or Python schema facades.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### WeakAnd, weighted-set, dot-product, and WAND queries (`query.weighted_and_wand`)

- The Python and TypeScript q facades do not yet expose

### userInput and userQuery text parsing (`query.user_input_user_query`)

- The parser intentionally supports a reduced grammar and rejects full Vespa escaping, boosts, range syntax, parser modes, and unsupported composites. No high-level facade currently exposes userInput/userQuery constructors.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### YQL query source (`query.yql`)

- Only the existing low-level parser subset is supported; full Vespa YQL grammar, query profiles, and search chains are outside the embedded wrapper. The public Rust Index has search()/browse() but no yql() entry point.

### Analyzed text search with default WeakAnd semantics (`query.text_weak_and`)

- Primary search(text) is analyzed text retrieval with Vespa WeakAnd-style default matching; it is not a phrase query. The low-level QueryText node is broader than the current facade helpers.

### Native in-term queries (`query.in_term`)

- The Python and TypeScript q facades do not yet expose

### NonEmpty query wrapper (`query.non_empty`)

- NonEmpty validates its lowered child and rejects statically empty boolean operators; it does not reproduce every parser-generated empty marker. No high-level facade helper exists yet.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Labeled query rank properties (`query.labels`)

- Labels are allocated by the adapter and exported as internal vespa.label.* rank properties; callers must not provide those properties. No high-level labeled-query helper exists yet.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

## Filter

### Structured filters (`filter.structured`)

- The Python high-level facade currently uses textual filters; it does not expose a typed structured-filter helper namespace.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Textual filter expressions (`filter.textual`)

- The filter expression language is a deliberate subset and is parsed and bound before entering canonical IR. The Rust SearchBuilder currently accepts typed Filter values only; it has no public textual-filter overload.

## Search

### Field sorting (`search.sorting`)

- The native hit ABI supports executing field sorts but does not expose Vespa sort values, so public Hit.sort_values is not populated by the real backend.

### First/last missing-value sort placement (`search.sort_missing_placement`)

- Python and TypeScript high-level sort() helpers

### Limit and offset pagination (`search.pagination`)

- Python additionally exposes page() as a convenience. Rust currently exposes limit/offset but no page() helper.

### Text search-field selection (`search.search_fields`)

- Field selection executes, but non-default per-field weights are rejected by the current native adapter. The Python high-level SearchBuilder does not yet expose search_field/search_fields.

## Result

### Document-summary field projection (`result.docsum_field_projection`)

- Stored scalar fields are projected through the current native summary seam. Named summary-class selection, rich docsum value decoding, and summary-feature readback are tracked separately.

### Named summary-class selection (`result.named_summary_class`)

- The public E2E proves default summary selection and field projection only. Native schema generation does not install application-defined named DocumentSummaryDefinition classes, so custom named summary execution is not yet available.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Rich document-summary values (scalar subset) (`result.rich_docsum_values`)

- Only scalar string, number, and boolean docsum values are executable through the current native result seam. Arrays, structs/maps, tensors, and other rich Vespa docsum values are not decoded faithfully.

### Summary-feature readback (`result.summary_features`)

- The portable Hit schema and IR represent summary features, but the current adapter leaves summary_features empty instead of decoding native summary-feature values.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Highlighting (`result.highlighting`)

- This pass supports Vespa-generated dynamic teaser output for stored, indexed text fields. The TypeScript SearchBuilder does not expose highlighting controls. Custom document-summary definitions, arbitrary Juniper rendering controls, grouped highlights, and rich docsum values remain separate capabilities.

### Dense tensor document readback (`result.tensor_document_readback`)

- Dense tensor fields round-trip through document GET/projection and reopen. Sparse/mixed document tensors are outside the supported high-level path; Python currently exposes readback as the tagged $searchkernel_tensor object rather than reconstructing a Tensor instance.

### Typed tensor feature readback (`result.tensor_feature_readback`)

- Tensor-backed match features currently cross the native result seam as opaque data bytes. The adapter does not decode them into a portable TensorValue.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

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

- The native grouping skeleton preserves summary() and finite max propagation, and the Python and TypeScript facades can construct agg.summary(), but grouped docsum payloads do not cross the native ABI. The Python E2E keeps this case explicitly blocked. Rust's current GroupingAggregation facade has no summary variant.

### Grouping continuations (`grouping.continuations`)

- Continuation tokens are preserved in GroupingRequestIr and in the portable result tree, but native grouping lowering rejects any request with continuations. Python and TypeScript result objects preserve tokens if received, yet there is no production native round-trip or high-level continuation request helper.

### Raw grouping source (`grouping.raw_source`)

- Raw grouping text is represented as caller intent, but resolve_grouping rejects it until a parser/normalizer can produce canonical structured grouping IR.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

## Update

### Document insert, upsert, patch, delete, and get (`update.document_crud`)

- This row covers the ordinary unconditional operations and Patch set/unset. Conditional mutations and advanced update-operation families are tracked separately. Ordered multiple updates targeting the same field are represented by Vespa's model but currently return Unsupported because the portable IR does not yet preserve that ordered field-update sequence.

### Arithmetic document updates (`update.arithmetic`)

- Increment, decrement, multiply, and divide execute through canonical FlatBuffers -> IR -> native DocumentUpdate in Python. The high-level Rust product API currently has Patch set/unset only and no advanced UpdateBuilder.

### Create-if-nonexistent document updates (`update.create_if_nonexistent`)

- Python UpdateBuilder.create() is proven through the real native path. The high-level Rust product API does not yet expose advanced document updates.

### Conditional document mutations (`update.conditional`)

- Conditions are preserved in canonical IR for upsert/delete/update, but the Vespa adapter returns conditional_mutation_not_supported. Python and TypeScript expose condition arguments/builders; Rust does not.

### Collection add/remove updates (`update.collection_add_remove`)

- AddUpdate and RemoveUpdate are represented in FlatBuffers, but update_to_ir currently rejects them as update_operation_not_supported before canonical IR.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Matched-element updates (`update.matched_element`)

- MatchUpdate is represented in FlatBuffers, but the current UpdateOperationIr has no matched-element variant and update_to_ir rejects it before canonical IR.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Tensor modify updates (`update.tensor_modify`)

- TensorModifyUpdate is represented in FlatBuffers, but there is no tensor-modify UpdateOperationIr and update_to_ir rejects it before canonical IR.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

## Tensor

### Dense tensor document values (`tensor.dense_document_crud`)

- Sparse and mixed tensors remain unsupported in the current high-level document and query-input paths.

## Schema

### Document-summary schema definitions (`schema.document_summaries`)

- Document-summary definitions and fields survive FlatBuffers-to-IR semantic conversion, but native schema generation does not install named document summaries for execution.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Array-of-struct document fields (`schema.array_struct_fields`)

- The portable/native path supports the existing array<object> shape used by SameElement. The high-level Rust Field, Python field-spec, and TypeScript schema facades expose only primitive/tensor kinds today; nested structs, maps, and heterogeneous object members remain unsupported.

### Position fields and derived geo attributes (`schema.position_geo_fields`)

- The native adapter publishes Vespa-compatible derived zcurve data and real geo queries are verified. Position is not exposed by the high-level Rust, Python, or TypeScript schema builders.

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
