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

Do not add one of these merely to complete M0–M2.
