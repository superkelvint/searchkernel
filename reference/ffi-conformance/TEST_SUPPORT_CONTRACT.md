# C5 verifier test-support contract

C5 does not have production handle creation yet. The verifier therefore requires a Rust-only, non-default feature:

```toml
ffi-test-support = []
```

When enabled, `searchkernel_ffi::test_support` must provide the following API. It is test infrastructure, not product API and must not export additional C symbols.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Insert,
    Upsert,
    Patch,
    Delete,
    Get,
    Search,
    Analyze,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DecodedResponse {
    Insert(searchkernel_ir::InsertResult),
    Upsert(searchkernel_ir::UpsertResult),
    Patch(searchkernel_ir::PatchResult),
    Delete(searchkernel_ir::DeleteResult),
    Get(searchkernel_ir::GetResult),
    Search(searchkernel_ir::SearchResult),
    Analyze(searchkernel_ir::AnalyzeResult),
    Error(searchkernel_ir::PortableError),
}

pub fn register_index(
    context: std::sync::Arc<searchkernel_ir::IndexContext>,
    backend: std::sync::Arc<dyn searchkernel_ir::Backend>,
) -> sk_handle_t;

pub fn live_handle_count() -> usize;
pub fn live_buffer_count() -> usize;

pub fn wrap_request(kind: RequestKind, inner_flatbuffer: &[u8]) -> Vec<u8>;

pub fn wrap_request_with_protocol(
    kind: RequestKind,
    inner_flatbuffer: &[u8],
    protocol_version: u32,
) -> Vec<u8>;

pub fn wrap_unknown_operation(
    operation_discriminant: u8,
    inner_flatbuffer: &[u8],
) -> Vec<u8>;

pub fn decode_response(bytes: &[u8]) -> Result<DecodedResponse, String>;
```

The helper functions above exist only so the checked-in verifier can construct
and inspect the normative FlatBuffers envelopes without making the production C
ABI larger.

The verifier still supplies the real C3 `RecordingBackend` / `ScriptedBackend` and independently computes expected IR through `searchkernel-ir`.

Production dispatch must not call into `test_support`.
