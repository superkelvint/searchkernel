# M14 — HNSW approximate nearest-neighbor search

## Goal

M14 freezes the first HNSW-backed approximate nearest-neighbor slice for the
embedded Vespa engine.

M12 proved dense query tensors and exact nearest-neighbor execution against an
attribute-only tensor. M13 proved dense tensor document CRUD and persistence.
M14 composes those capabilities with a real Vespa HNSW tensor index.

The accepted low-level path is:

```text
searchkernel-vespa-engine Schema
        ↓
ConfigCompiler
        ↓
tensor attribute + distance metric + HNSW config
        ↓
real Proton tensor attribute / HNSW graph
        ↓
Query::NearestNeighbor{... approximate=false}
        or
Query::NearestNeighborWithOptions{... approximate=true}
        ↓
real Proton Matcher
```

The accepted portable path is:

```text
IndexDefinition with IndexConfig.hnsw + AttributeConfig.distance_metric
        ↓
searchkernel-ir (*_to_ir)
        ↓
searchkernel-ir-adapter::compile_index_definition
        ↓
the same native HNSW schema/configuration
        ↓
portable typed nearest-neighbor query annotations
        ↓
real Proton Matcher
```

M13 remains cumulative and unchanged.

## Frozen low-level schema API

The compile-only API contract is:

```text
reference/vespa/api-contract/src/bin/m14.rs
```

M14 adds these low-level schema concepts:

```rust
DistanceMetric
HnswIndexConfig

Field::distance_metric(...)
Field::hnsw(...)
```

The tensor field remains an attribute. `Field::hnsw(...)` is a distinct tensor
index capability and MUST NOT be implemented by treating the tensor as a text
`Field::indexed()` field or by emitting a tensor entry in `indexschema.cfg`.

The frozen construction controls are:

```text
max_links_per_node
neighbors_to_explore_at_insert
```

No additional backend-specific HNSW construction knobs are part of M14.

## Frozen distance metric

M14 execution freezes `EUCLIDEAN` because M12 already proved its query-tensor
semantics and it keeps the behavioral oracle easy to inspect.

The low-level `DistanceMetric` API MUST nevertheless represent all distance
metrics already present in the canonical portable contract:

```text
Euclidean
Angular
DotProduct
PrenormalizedAngular
GeoDegrees
Hamming
```

M14 acceptance executes Euclidean and structurally verifies that the adapter
contains an exhaustive portable-to-native metric mapping. Later milestones may
add behavioral fixtures for the remaining metrics without redesigning this API.

## Required generated attribute configuration

For the frozen fixture:

```text
field                  embedding
tensor type            tensor<float>(x[3])
distance metric        EUCLIDEAN
max links per node     4
neighbors at insert    16
```

`attributes.cfg` MUST contain semantics equivalent to:

```text
attribute[N].name "embedding"
attribute[N].datatype TENSOR
attribute[N].collectiontype SINGLE
attribute[N].tensortype "tensor<float>(x[3])"
attribute[N].distancemetric EUCLIDEAN
attribute[N].index.hnsw.enabled true
attribute[N].index.hnsw.maxlinkspernode 4
attribute[N].index.hnsw.neighborstoexploreatinsert 16
```

The verifier canonicalizes these keys instead of freezing unrelated Vespa
configuration defaults.

The same tensor MUST NOT appear as a normal inverted index field in
`indexschema.cfg`.

## Query controls

M14 uses the already-existing nearest-neighbor request controls:

```text
target hits
approximate
explore additional hits
distance threshold
```

No new native query ABI is required by the contract.

The verifier must exercise the same HNSW-enabled field with:

```text
approximate = false   exact/brute-force control
approximate = true    HNSW ANN
approximate = true + exploreAdditionalHits
approximate = true + distanceThreshold
```

`approximate=false` remains valid even when an HNSW graph exists.

## Behavioral acceptance

The frozen corpus contains a document whose vector is identical to the query.
Every positive query must return that document as the first hit.

M14 deliberately does **not** freeze the complete ANN hit ordering or an ANN
recall percentage. HNSW is approximate and full ordering is not a stable
portable contract.

For each query step the verifier freezes only:

```text
top_hit
returned hit count
whether forbidden/deleted IDs occur
```

The direct C++ oracle and both Search Kernel paths must agree on those frozen
invariants.

## Mutable HNSW lifecycle

M14 must additionally prove HNSW graph maintenance through real document
mutation and persistence:

```text
insert corpus
ANN query -> a is top
assign a new far-away vector
ANN query -> b is top
close
reopen
ANN query -> b is still top
delete b
ANN query -> b is absent
close
reopen
ANN query -> b remains absent
```

No Rust-side graph, shadow vector store, or rebuild-on-query shortcut is
allowed.

## Portable schema support

The existing portable fields are normative:

```text
IndexConfig.hnsw
HnswIndexConfig.max_links_per_node
HnswIndexConfig.neighbors_to_explore_at_insert
AttributeConfig.distance_metric
```

M14 requires `searchkernel-ir-adapter::compile_index_definition()` to lower
those existing IR values instead of returning `schema_feature_not_supported`.

No FlatBuffers schema change is required by M14.

## Validation ownership

### Portable IR owns

Existing canonical validation remains authoritative, including:

```text
HNSW on a non-tensor field -> invalid_type_combination
distance metric on a non-tensor field -> invalid_type_combination
```

### IR adapter owns

The adapter owns lossless lowering of supported HNSW/distance-metric IR into
the low-level native schema model. It must reject unsupported opaque HNSW or
attribute options rather than silently ignore them.

### Native M layer owns

The low-level schema compiler owns native constraints for its schema model:

```text
HNSW requires a tensor field
HNSW requires an attribute-backed tensor
max_links_per_node must be positive when supplied
neighbors_to_explore_at_insert must be positive when supplied
distance metric is valid only for tensor fields
```

It also owns faithful `attributes.cfg` emission.

## Frozen negative categories

M14 freezes these coarse categories:

```text
hnsw_requires_tensor
hnsw_requires_attribute
invalid_hnsw_max_links
invalid_hnsw_neighbors_at_insert
distance_metric_requires_tensor
portable_hnsw_requires_tensor
portable_distance_metric_requires_tensor
hnsw_options_not_supported
attribute_options_not_supported
```

Human-readable messages are not frozen.

## Independent oracle

Positive execution must be accepted against a direct C++ oracle built from the
pinned Vespa checkout.

The oracle:

- writes the HNSW tensor attribute configuration directly;
- feeds real `TensorFieldValue` documents through Proton persistence;
- builds exact and approximate `NearestNeighborTerm` query trees directly;
- executes `DocumentDB::match()`;
- updates and deletes tensor documents through the real persistence API;
- closes/reopens the same persistent index for lifecycle checks;
- does not call Search Kernel production code.

The config semantics are additionally frozen by inspecting Search Kernel's
generated `attributes.cfg`, so an implementation cannot pass by using an exact
attribute-only fixture.

## Forbidden shortcuts

M14 fails if production code:

```text
computes vector distance or ANN in Rust
stores a parallel HNSW/vector graph outside Vespa
falls back to exact scan while discarding the approximate flag
ignores HNSW construction parameters
ignores the configured distance metric
uses tensor Field::indexed() as a text-index shortcut
hard-codes verifier document IDs or expected top hits
invokes the direct oracle from production code
uses a fake/scripted backend for acceptance
```

The verifier also checks that `allow_approximate` reaches Vespa's native query
builder and that HNSW configuration is emitted by the native schema compiler.

## Explicit non-goals

Not required by M14:

```text
multi-vector / mapped-dimension HNSW
sparse or mixed query tensors
recall/latency benchmarking
adaptive global-filter HNSW policy controls
prefetch_tensors
target_hits_max_adjustment_factor
filter-first HNSW policy knobs
multithreaded-indexing as a public Search Kernel knob
Python/TypeScript ergonomic schema sugar
all distance metrics exercised behaviorally
```

Those are separate capability or product-API work.
