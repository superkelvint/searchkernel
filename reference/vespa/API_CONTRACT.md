# Rust API contract — M0–M2

This document freezes the public Rust API required by the first three
milestones.

The implementation may add private helpers. It must not require callers to use
additional public concepts to complete the scenarios below.

Do not redesign this API while implementing a milestone. API changes are a
separate maintainer decision and require updating the compile fixtures before
implementation begins.

## M0

M0 has no public Rust API requirement. It establishes the native SDK and private
C ABI defined by `NATIVE_ABI_CONTRACT.h`.

## M1 — engine ownership

The crate name is:

```text
searchkernel
```

The following public surface must exist:

```rust
pub type Result<T> = std::result::Result<T, Error>;

pub struct Error { /* private representation */ }

pub struct EngineConfig { /* private representation */ }

impl EngineConfig {
    pub fn from_directory(path: impl AsRef<std::path::Path>) -> Result<Self>;
}

pub struct Engine { /* private representation */ }

impl Engine {
    pub fn open(
        data_dir: impl AsRef<std::path::Path>,
        config: EngineConfig,
    ) -> Result<Self>;
}
```

`Error` must implement:

```rust
std::fmt::Debug
std::fmt::Display
std::error::Error
Send
Sync
'static
```

`Engine` owns exactly one native engine handle. Dropping `Engine` closes that
handle exactly once.

No public Vespa C++ type or raw pointer is exposed.

### Why `EngineConfig::from_directory` exists

This is intentionally native-oriented. M0–M2 consume the known Vespa config
fixture rather than inventing Search Kernel schema semantics prematurely.

A future milestone may add a nicer schema/configuration surface. That does not
change the M1/M2 verification contract unless explicitly revised.

## M2 — mutation and search

M2 adds these public types:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DocumentId { /* private representation */ }

impl DocumentId {
    pub fn parse(value: impl Into<String>) -> Result<Self>;
    pub fn as_str(&self) -> &str;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
}

pub struct Document { /* private representation */ }

impl Document {
    pub fn new(id: DocumentId) -> Self;
    pub fn set_field(&mut self, name: impl Into<String>, value: Value);
}

pub struct DocumentUpdate { /* private representation */ }

impl DocumentUpdate {
    pub fn new(id: DocumentId) -> Self;
    pub fn assign(&mut self, name: impl Into<String>, value: Value);
}

pub struct Query { /* private representation */ }

impl Query {
    pub fn term(field: impl Into<String>, value: Value) -> Self;
}

pub struct SearchRequest { /* private representation */ }

impl SearchRequest {
    pub fn new(query: Query) -> Self;
    pub fn limit(self, limit: usize) -> Self;
}

pub struct Hit { /* private representation */ }

impl Hit {
    pub fn id(&self) -> &DocumentId;
}

pub struct SearchResult { /* private representation */ }

impl SearchResult {
    pub fn hits(&self) -> &[Hit];
}
```

`Engine` additionally provides:

```rust
impl Engine {
    pub fn put(&mut self, document: Document) -> Result<()>;

    pub fn update(&mut self, update: DocumentUpdate) -> Result<()>;

    /// Returns whether a live document was removed.
    pub fn remove(&mut self, id: &DocumentId) -> Result<bool>;

    pub fn search(&self, request: SearchRequest) -> Result<SearchResult>;
}
```

## Deliberately not specified yet

M0–M2 do not define:

```text
Schema
Collection
Analyzer
text query syntax
rank profile API
facets
sorting
filters beyond the one integer term fixture
async API
thread-sharing guarantees
serde representation
Python API
```

Here, “async API” refers to the embedded Vespa engine and high-level Rust
surface. The portable client may provide a separate async transport/client
surface; it must not make `Backend`, `Index`, or embedded engine execution
asynchronous.

Do not add one of these merely to complete M0–M2.

---

# M10 additions — ranking integration

M10 extends the generic low-level M-series API. It does not introduce a
milestone-specific ranking request/result family.

```rust
use searchkernel_vespa_engine::{Field, FieldType, Query, RankProfile, Schema, SearchRequest};

let schema = Schema::new("typea")
    .field(Field::new("title", FieldType::Text).indexed().summary())
    .field(Field::new("popularity", FieldType::Integer).attribute().summary())
    .rank_profile(
        RankProfile::new("product")
            .first_phase("bm25(title) + query(boost) * attribute(popularity)")
            .match_feature("bm25(title)")
            .match_feature("query(boost)")
            .match_feature("attribute(popularity)"),
    );

let request = SearchRequest::new(Query::term("title", "fox"))
    .rank_profile("product")
    .query_feature("boost", 2.0)
    .limit(10);
```

The required new public shape is:

```text
RankProfile::new(name)
RankProfile::first_phase(expression)
RankProfile::match_feature(feature)
Schema::rank_profile(profile)
SearchRequest::query_feature(name, finite_f64)
SearchRequest::rank_property(name, value)
Hit::match_features: BTreeMap<String, MatchFeatureValue>
```

The expression and feature strings are Vespa ranking syntax. The Rust wrapper
validates wrapper-owned structural constraints but does not parse or evaluate
ranking expressions.

`SearchRequest::query_feature` is scalar-only in M10 v1. Tensor query inputs are
not part of this contract.

The same effective property name may not be supplied through both
`query_feature` and `rank_property` in one request.

Search Kernel reserves the low-level Vespa property
`vespa.matching.numthreadspersearch` and supplies the deterministic value `1`
to its embedded matcher. The key is reserved in the native rank-property
namespace and callers must not set it through either `query_feature` or
`rank_property`; doing so is an `invalid_query_feature` or
`invalid_rank_property` error, respectively.

`MatchFeatureValue` preserves the native Vespa feature boundary:

```text
Double(f64) | Data(Vec<u8>)
```

The native `DATA` bytes are opaque and copied out of the result by the safe
wrapper. Tensor recognition or decoding, if supported later, belongs above
this representation; the Vespa feature container itself is not a
double-or-tensor container.

---

# M14 additions — HNSW approximate nearest-neighbor indexing

M14 extends the low-level tensor-field schema with the minimum configuration
required to build and search Vespa's native HNSW index.  It deliberately does
not redefine nearest-neighbor query semantics: M12's existing
`NearestNeighborOptions` remains the query-side contract.

The required new public shape is:

```rust
use searchkernel_vespa_engine::{DistanceMetric, Field, FieldType, HnswIndexConfig};

let field = Field::new(
    "embedding",
    FieldType::Tensor("tensor<float>(x[3])".into()),
)
.attribute()
.distance_metric(DistanceMetric::Euclidean)
.hnsw(
    HnswIndexConfig::new()
        .max_links_per_node(4)
        .neighbors_to_explore_at_insert(16),
);
```

The M14 schema API therefore adds:

```text
DistanceMetric
    Euclidean
    Angular
    DotProduct
    PrenormalizedAngular
    GeoDegrees
    Hamming

HnswIndexConfig::new()
HnswIndexConfig::max_links_per_node(u32)
HnswIndexConfig::neighbors_to_explore_at_insert(u32)
Field::distance_metric(DistanceMetric)
Field::hnsw(HnswIndexConfig)
```

`Field::indexed()` remains the ordinary inverted-index capability used by text
fields.  HNSW is explicit tensor-index configuration and must not be smuggled
through the text-index boolean.

M14 requires:

```text
HNSW -> tensor field + attribute
DistanceMetric -> tensor field
max_links_per_node > 0
neighbors_to_explore_at_insert > 0
```

The portable FlatBuffers/IR types already represent HNSW and distance metrics;
M14 adds no new portable schema and no new native nearest-neighbor query ABI.
