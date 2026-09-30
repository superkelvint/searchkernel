# Search Kernel limitations

This file is generated from [`docs/capabilities.json`](docs/capabilities.json).
Search Kernel is a thin wrapper around the pinned Vespa engine, but its
public contract is intentionally smaller than Vespa's complete model.
Unsupported behavior must remain an explicit error; it must not be silently
approximated.

**Snapshot:** 2026-09-30

## Release scope

- The `0.1.x beta` release is qualified only for **linux x86_64 glibc**.
- Supported release surfaces: `rust-embedded` (`native`), `searchkerneld` (`native`), `python` (`managed-local`), `python` (`connect`), `typescript-node` (`connect`), `ruby` (`connect`).
- Deferred release surfaces: `typescript-browser-grpc-web`, `php`, `ruby-managed-local`, `go`, `macos`, `windows`, `linux-aarch64`.
- Implementation presence outside this matrix is not a release-support promise; see [`docs/release-support.md`](docs/release-support.md).

## Query

### True and false query constants (`query.boolean_constants`)

- Rust exposes true/false query constants; the Python and TypeScript q facades do not yet expose equivalent helpers.

### Comparison queries (`query.comparison`)

- Python and TypeScript expose equality comparison helpers, but their high-level q facades do not yet expose the full comparison-operator surface.

### sameElement query (`query.same_element`)

- Requires supported array-of-struct fields and native elementFilter; it is not a general nested struct or map model. Python and TypeScript expose the supported array-of-struct schema/query path; the high-level Rust schema facade still cannot define the required array-of-struct field.

### Alternatives queries (`query.alternatives`)

- Alternatives currently executes only when every child is a term; general alternatives are rejected by the adapter. High-level Rust, Python, and TypeScript facades do not expose this node.

### Nearest-neighbor vector search (`query.nearest_neighbor`)

- The proven public path uses dense tensor query inputs and exposes only target hits, approximate mode, distance threshold, and explore-additional-hits rather than the full Vespa HNSW tuning surface.

### Geo-location and bounding-box queries (`query.geo`)

- Real Vespa execution is verified, including inclusive bounding-box edges. Python and TypeScript expose position-field schema builders and geo query helpers; the high-level Rust schema facade does not yet expose position fields.

### Predicate queries (`query.predicate`)

- Only the supported predicate feature/range representation and documented expression shapes are lowered. Predicate field definitions are not exposed by the high-level Rust or Python schema facades.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Equivalence queries (`query.equivalence`)

- No high-level Rust, Python, or TypeScript facade helper currently constructs equivalence queries.

### Rank query composition (`query.rank`)

- The TypeScript q facade does not yet expose rank composition.

### userInput and userQuery text parsing (`query.user_input_user_query`)

- The parser intentionally supports only a reduced grammar; full Vespa escaping, boosts, range syntax, parser modes, and unsupported composites are rejected. No high-level facade currently exposes userInput/userQuery constructors.

### YQL query source (`query.yql`)

- Only the existing low-level parser subset is supported; full Vespa YQL grammar, query profiles, and search chains are outside the embedded wrapper. Rust exposes Index::yql() through the same canonical search resolver, including scalar and tensor parameter bindings.

### Analyzed text search with default WeakAnd semantics (`query.text_weak_and`)

- Primary search(text) is analyzed text retrieval with Vespa WeakAnd-style default matching; it is not a phrase query. The low-level QueryText node is broader than the current facade helpers.

### Typed analyzed-text query nodes (`query.typed_text`)

- The typed QueryText node executes through the adapter, but no high-level Rust, Python, or TypeScript q facade helper constructs it.

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

- Missing-value placement is represented and executable, but the Python and TypeScript high-level field-sort helpers do not yet expose first/last placement controls.

### Limit and offset pagination (`search.pagination`)

- Python and Rust expose page() as a convenience over limit/offset.

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

- Rust, Python, and TypeScript preserve canonical summary-feature values when they are present in a portable or scripted Hit, but the current native adapter leaves summary_features empty instead of decoding native summary-feature values.
- Rank-feature projection is likewise preserved when present, but the current native adapter does not populate rank_features.

### Highlighting (`result.highlighting`)

- This pass supports Vespa-generated dynamic teaser output for stored, indexed text fields. The TypeScript SearchBuilder does not expose highlighting controls. Custom document-summary definitions, arbitrary Juniper rendering controls, grouped highlights, and rich docsum values remain separate capabilities.

### Dense tensor document readback (`result.tensor_document_readback`)

- Dense tensor fields round-trip through document GET/projection and reopen. Sparse/mixed document tensors are outside the supported high-level path; Python currently exposes readback as the tagged $searchkernel_tensor object rather than reconstructing a Tensor instance.

### Typed tensor feature readback (`result.tensor_feature_readback`)

- Tensor-backed match features currently cross the native result seam as opaque data bytes. The adapter does not decode them into a portable TensorValue.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

### Match-feature readback (`result.match_features`)

- Scalar native match features are decoded. Rust, Python, and TypeScript preserve typed canonical match, summary, and rank feature maps when those values are present in a portable Hit. Tensor-backed native match-feature output remains opaque bytes, and the native adapter currently leaves summary_features and rank_features unpopulated.

### Recursive grouping result trees (`result.grouping_tree`)

- The canonical IR/result protocol is a heterogeneous recursive ResultNode tree. Rust now publicly exports SearchResult.root plus ResultNode, ResultGroup, ResultHit, and ResultLimits with mixed child ordering preserved; convenience hit/group projections intentionally separate node kinds and should not be used when canonical interleaving matters.

### Coverage and degradation metadata (`result.degradation_metadata`)

- The real backend populates coverage counters and match-phase/timeout/ANN degradation flags. Portable timing, trace, arbitrary metadata, and native Vespa UniqueIssues are not currently populated by this backend; UniqueIssues are deliberately not fabricated into SearchResult.errors.

## Grouping

### Structured grouping and faceting (`grouping.structured`)

- The native lowering is a linear grouping plan and cannot execute arbitrary parallel grouping branches from the full portable tree. Rust exposes root outputs, nested grouping, aggregation aliases, and regex keep filters.

### Executable grouping expression/function subset (`grouping.functions`)

- The native adapter supports attributes, literals, arithmetic, aggregation references, aliases, and the verified function subset. Member, MapLookup, and Bucket remain rejected. Rust exposes named aggregation aliases and regex keep filters; Python exposes the richer expression facade.

### Grouping aggregations (`grouping.aggregations`)

- The native layer supports count, average, sum, min, max, stddev, xor, and quantiles. Rust exposes those aggregation constructors, aliases, ordering, precision, and root outputs; richer grouping expression/filter ergonomics remain narrower than the portable model. Summary aggregation is tracked separately because grouped docsum payloads are incomplete.

### Summary aggregation (`grouping.summary_aggregation`)

- The native grouping skeleton preserves summary() and finite max propagation, and the Rust, Python, and TypeScript facades can construct summary aggregation, but grouped docsum payloads do not cross the native ABI; production real-engine use remains explicitly blocked.

### Grouping continuations (`grouping.continuations`)

- Continuation tokens are preserved in GroupingRequestIr and in the portable result tree, but native grouping lowering rejects any request with continuations. Python and TypeScript result objects preserve tokens if received, yet there is no production native round-trip or high-level continuation request helper.

### Raw grouping source (`grouping.raw_source`)

- Raw grouping text is represented as caller intent, but resolve_grouping rejects it until a parser/normalizer can produce canonical structured grouping IR.
- The TypeScript facade does not expose this capability as an ergonomic helper; generated low-level FlatBuffers remain available.

## Update

### Document insert, upsert, patch, delete, and get (`update.document_crud`)

- This row covers the ordinary unconditional operations and Patch set/unset. Conditional mutations and advanced update-operation families are tracked separately. Ordered multiple updates targeting the same field are represented by Vespa's model but currently return Unsupported because the portable IR does not yet preserve that ordered field-update sequence.

### Arithmetic document updates (`update.arithmetic`)

- Increment, decrement, multiply, and divide execute through canonical FlatBuffers -> IR -> native DocumentUpdate. Rust exposes assignment, clear, and each arithmetic operator; multiple updates to the same field in one request remain unsupported by the shared resolver.

### Create-if-nonexistent document updates (`update.create_if_nonexistent`)

- Python UpdateBuilder.create() is proven through the real native path. Rust exposes set, unset, and arithmetic updates but not create-if-nonexistent.

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

- The portable/native path supports the existing array<object> shape used by SameElement. Python and TypeScript expose the supported array-of-struct schema shape; the high-level Rust schema facade does not yet define it. Nested maps and arbitrary heterogeneous object models remain unsupported.

### Position fields and derived geo attributes (`schema.position_geo_fields`)

- The native adapter publishes Vespa-compatible derived zcurve data and real geo queries are verified. Python and TypeScript expose position fields; the high-level Rust schema builder does not yet.

## Ranking

### Rank-profile selection (`ranking.profile_selection`)

- The profile must exist in the compiled index schema; Search Kernel does not introduce a second ranking language.

### Numeric scalar query inputs (`ranking.numeric_query_inputs`)

- The Rust facade now represents bool, integer, unsigned integer, float, string, JSON, tensor, and expression query inputs. The embedded native query-feature seam executes finite lossless numeric scalars (and dense tensors through the tensor path); expression inputs are preserved canonically but return query_input_expression_not_supported.

### Dense tensor query inputs (`ranking.tensor_query_inputs`)

- Only literal dense tensors are executable by the current native query-input seam; sparse/mixed tensors and expression inputs are rejected.

### Rank properties (`ranking.rank_properties`)

- Adapter-owned vespa.label.* properties are reserved for labeled queries and cannot be supplied by callers.

## Platform

### Embedded single-node Vespa scope (`platform.embedded_vespa_scope`)

- Distributed routing, configuration services, schema deployment, federation, and other container-level Vespa facilities are outside this wrapper.

## Daemon

### Targeted multi-index daemon lifecycle protocol (`daemon.multi_index_lifecycle`)

- Target-less envelopes remain valid only for embedded/per-index callers; multi-index daemon ingress reports a canonical missing-target error.

## Evidence and versioning

The state and limitations are cross-layer summaries. Behavioral authority
remains the frozen contract, pinned Vespa source, direct oracle, and
acceptance verifier in that order. Vespa behavior is pinned to the revision
recorded in [`reference/vespa/PINNED_VESPA.json`](reference/vespa/PINNED_VESPA.json).
