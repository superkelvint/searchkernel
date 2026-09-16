# C6 — BFBS-driven generation and portable client core

This directory freezes the C6 acceptance contract.

## Normative source and BFBS compiler IR

The checked-in FlatBuffers schemas under `api/` are normative. The pinned FlatBuffers compiler is exactly `25.12.19`.

```text
api/searchkernel_ffi_request.fbs
api/searchkernel_ffi_response.fbs
       │
       │ flatc 25.12.19 -b --schema
       ▼
core/crates/searchkernel-json/bfbs/
       │
       ▼
searchkernel-bindgen
```

BFBS is the machine-readable compiler IR consumed by C6 generation. `searchkernel-bindgen` must not implement another `.fbs` parser.

The verifier proves this behaviorally: it creates schema material that does not exist in the repository, compiles it to BFBS, deletes the mutated `.fbs` files, then requires bindgen to generate the synthetic operation and field.

## `flatc` selection

Verifier discovery order:

1. `FLATC_PATH`;
2. `flatc` on `PATH`;
3. executable files named `flatc` under the repository `target/` tree.

The selected executable must report exactly `25.12.19`.

The verifier owns `.fbs → BFBS`. Bindgen receives BFBS only.

## Required bindgen CLI

The workspace contains `tools/searchkernel-bindgen/`, package name `searchkernel-bindgen`.

Frozen commands:

```text
searchkernel-bindgen generate \
  --bfbs-root <bfbs-directory> \
  --output-root <empty-output-directory>
```

For the canonical fixture corpus:

```text
searchkernel-bindgen generate \
  --bfbs-root <bfbs-directory> \
  --fixture-root <directory-containing-requests-and-responses> \
  --output-root <empty-output-directory>
```

`--fixture-root` is optional so synthetic BFBS can be tested without synthetic binary fixtures.

Rules:

- `--output-root` mirrors generated repository-relative paths;
- generation must not modify the repository when `--output-root` is supplied;
- no `.fbs` or repository-root argument is supplied to bindgen;
- malformed/incomplete BFBS exits non-zero;
- unsupported or incomplete schema semantics fail explicitly rather than silently omitting output.

## Required generated paths

`GENERATED_PATHS.txt` freezes the checked-in generated tree:

```text
core/crates/searchkernel-client/src/generated.rs
core/crates/searchkernel-json/src/generated.rs
reference/json-conformance/generated/
```

The generated reference directory contains at least:

```text
manifest.json
operation-map.json
searchkernel.schema.json
openapi-components.json
API.md
requests/*.json
responses/*.json
```

Generated JSON fixture names correspond one-for-one with canonical `.skrq` / `.skrs` fixtures.

## Generated manifest

`manifest.json` includes:

```json
{
  "generated_by": "searchkernel-bindgen",
  "format_version": 1,
  "bfbs": {
    "searchkernel_ffi_request.bfbs": {"sha256": "..."},
    "searchkernel_ffi_response.bfbs": {"sha256": "..."}
  }
}
```

Hashes must equal the exact committed BFBS bytes.

## Generated portable client API

C6 provides `core/crates/searchkernel-client/` with this frozen minimum public API:

```rust
pub trait Transport {
    type Error;
    fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncTransport {
    type Error;
    async fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

pub enum ClientError<E> {
    Transport(E),
    Portable(model::Error),
    Protocol(String),
}

pub struct Client<T: Transport> { /* ... */ }

pub struct AsyncClient<T: AsyncTransport> { /* ... */ }

impl<T: Transport> Client<T> {
    pub fn new(transport: T) -> Self;

    pub fn insert(&self, request: model::InsertRequest)
        -> Result<model::InsertResult, ClientError<T::Error>>;
    pub fn upsert(&self, request: model::UpsertRequest)
        -> Result<model::UpsertResult, ClientError<T::Error>>;
    pub fn patch(&self, request: model::PatchRequest)
        -> Result<model::PatchResult, ClientError<T::Error>>;
    pub fn delete(&self, request: model::DeleteRequest)
        -> Result<model::DeleteResult, ClientError<T::Error>>;
    pub fn get(&self, request: model::GetRequest)
        -> Result<model::GetResult, ClientError<T::Error>>;
    pub fn search(&self, request: model::SearchRequest)
        -> Result<model::SearchResult, ClientError<T::Error>>;
    pub fn analyze(&self, request: model::AnalyzeRequest)
        -> Result<model::AnalyzeResult, ClientError<T::Error>>;
    pub fn describe(&self, request: model::DescribeRequest)
        -> Result<model::DescribeResult, ClientError<T::Error>>;
    pub fn inspect_query(&self, request: model::InspectQueryRequest)
        -> Result<model::InspectQueryResult, ClientError<T::Error>>;
}

impl<T: AsyncTransport> AsyncClient<T> {
    pub fn new(transport: T) -> Self;

    pub async fn insert(&self, request: model::InsertRequest)
        -> Result<model::InsertResult, ClientError<T::Error>>;
    pub async fn upsert(&self, request: model::UpsertRequest)
        -> Result<model::UpsertResult, ClientError<T::Error>>;
    pub async fn patch(&self, request: model::PatchRequest)
        -> Result<model::PatchResult, ClientError<T::Error>>;
    pub async fn delete(&self, request: model::DeleteRequest)
        -> Result<model::DeleteResult, ClientError<T::Error>>;
    pub async fn get(&self, request: model::GetRequest)
        -> Result<model::GetResult, ClientError<T::Error>>;
    pub async fn search(&self, request: model::SearchRequest)
        -> Result<model::SearchResult, ClientError<T::Error>>;
    pub async fn analyze(&self, request: model::AnalyzeRequest)
        -> Result<model::AnalyzeResult, ClientError<T::Error>>;
    pub async fn describe(&self, request: model::DescribeRequest)
        -> Result<model::DescribeResult, ClientError<T::Error>>;
    pub async fn inspect_query(&self, request: model::InspectQueryRequest)
        -> Result<model::InspectQueryResult, ClientError<T::Error>>;
}
```

Generated request models implement `Default`; semantic validity remains C2's concern.

Every client operation performs exactly one `Transport::call`, emits the matching SKRQ variant, accepts the matching SKRS response, and maps portable `Error` to `ClientError::Portable`.

The async client has the same operation, encoding, decoding, and error
semantics, performing exactly one `AsyncTransport::call` per operation. The
async trait is runtime-neutral and is intended for generic clients; this
contract does not require an executor or make the synchronous transport
blocking work asynchronous.

## Operation coverage

`expected/operation-map.json` freezes:

```text
insert        InsertRequest       → InsertResult
upsert        UpsertRequest       → UpsertResult
patch         PatchRequest        → PatchResult
delete        DeleteRequest       → DeleteResult
get           GetRequest          → GetResult
search        SearchRequest       → SearchResult
analyze       AnalyzeRequest      → AnalyzeResult
describe      DescribeRequest     → DescribeResult
inspect_query InspectQueryRequest → InspectQueryResult
```

`Error` is the common portable error response.

These surfaces remain complete together:

```text
RequestPayload / ResponsePayload
operation map
generated Client
generated JSON envelope
shared dispatcher
metadata/docs
```

## Canonical Search Kernel JSON

Official FlatBuffers JSON is not public Search Kernel JSON.

Request:

```json
{"protocol_version":1,"request":{"search":{"query":"foo"}}}
```

Response:

```json
{"protocol_version":1,"response":{"search":{}}}
```

Portable error uses the one-key `error` response variant.

### Symbolic unions

Every FlatBuffers union is a symbolic one-key object, for example:

```json
{"string":"x"}
{"term_query":{"field":"title","value":{"string":"rust"}}}
```

Zero, multiple, or unknown variant keys are rejected. Raw wire discriminator fields such as `payload_type`, `kind_type`, `expr_type`, `policy_type`, `operation_type`, and `target_type` are forbidden in public JSON.

### Scalars

```text
bool                       JSON boolean
int32/uint32/bounded count JSON number
finite float32/float64     JSON number
int64                      canonical decimal JSON string
uint64                     canonical decimal JSON string
bytes                      base64 JSON string
```

64-bit integer strings are base 10, have no leading `+`, no unnecessary leading zero, no exponent/decimal point, reject `-0`, and enforce full ranges. NaN and ±Infinity are rejected.

### Presence and strictness

Preserve absent vs explicit false/zero/empty string/empty collection. JSON `null` is accepted only for semantic null.

Reject unknown fields, duplicate fields, unknown enum names, invalid union forms, out-of-range or non-canonical integer strings, malformed base64, invalid null, and raw official-FlatBuffers JSON.

## Generated metadata and docs

`searchkernel.schema.json` and `openapi-components.json` are valid JSON, carry `x-searchkernel-generated-by: searchkernel-bindgen`, contain all current operations, and describe canonical Search Kernel JSON rather than raw FlatBuffers discriminator mechanics.

`openapi-components.json` contains non-empty `components.schemas`.

`API.md` carries `@generated by searchkernel-bindgen`, documents every operation, and documents one-key unions, decimal-string int64/uint64, and base64 bytes.

These outputs are derived documentation, not a second normative contract.

## Deterministic generation

The verifier runs bindgen twice in clean output directories. Trees must be byte-for-byte identical and match checked-in generated outputs. No timestamps, absolute paths, random ordering, or host-specific data are permitted.

## BFBS-only mutation proof

The verifier adds temporary:

```text
VerifierProbeRequest
VerifierProbeResult
SearchRequest.verifier_probe_field
```

It compiles the temporary schemas to BFBS, deletes the `.fbs` sources, copies the bindgen executable into an isolated temp directory, and generates using only BFBS.

Required output includes operation `verifier_probe`, `Client::verifier_probe`, and JSON/schema metadata containing `verifier_probe_field`.

## Fail-closed mutation proof

The verifier creates `VerifierOrphanRequest` with no result pair. Bindgen must fail non-zero and identify the orphan in its diagnostic. Malformed BFBS must also fail non-zero.

## Existing semantic authority

`reference/api-conformance` remains authoritative for FlatBuffers → `*_to_ir` → Search Kernel IR. Existing native-vs-JSON convergence tests remain required and must compare identical recorded IR.

## Forbidden shortcuts

Forbidden:

- reparsing `.fbs` inside bindgen;
- hardcoded current operation behavior;
- handwritten per-message JSON mapping;
- JSON → IR direct conversion;
- a second JSON semantic dispatcher;
- public official FlatBuffers JSON;
- silently skipped schema constructs;
- generation succeeding while omitting an operation;
- changing semantic corpus expectations merely to make C6 pass.
