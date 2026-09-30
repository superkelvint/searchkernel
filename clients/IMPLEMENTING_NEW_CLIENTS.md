# Implementing New Search Kernel Clients

**Status:** Normative implementation SOP  
**Audience:** Agents and engineers adding a first-party non-Rust Search Kernel SDK  
**Applies to:** Python, TypeScript/Node, Go, Java, PHP, C#, Ruby, Swift, and future non-Rust clients  
**Does not apply to:** the direct in-process Rust API

---

## 1. Purpose

This document is the standard operating procedure for implementing a new first-party Search Kernel client.

The goal is not merely to make another language able to send requests. The goal is to add a language facade without creating another implementation of Search Kernel semantics.

Every non-Rust client must preserve one of the sanctioned representation paths
below and converge on the same canonical dispatcher.

Default binary/native path:

```text
idiomatic language facade
        ↓
flatc-generated FlatBuffer models
        ↓
canonical RequestEnvelope bytes
        ↓
generated gRPC transport
        ↓
searchkerneld
        ↓
shared dispatcher
        ↓
searchkernel-ir / *_to_ir()
        ↓
backend
```

Explicit HTTP-first path:

```text
idiomatic language facade
        ↓
standard FlatBuffers JSON RequestEnvelope
        ↓
generic HTTP execute endpoint
        ↓
official BFBS-backed FlatBuffers JSON parser
        ↓
canonical RequestEnvelope bytes
        ↓
the same shared dispatcher
        ↓
searchkernel-ir / *_to_ir()
        ↓
backend
```

HTTP-first is a representation/transport choice, not a second Search Kernel API.

The non-negotiable invariant is:

> **FlatBuffers defines the portable domain contract. `searchkernel-ir` defines portable semantics. A language SDK owns ergonomics and transport plumbing only.**

A new client must not introduce a second domain model, a second filter parser, a second semantic validator, or a second interpretation of Search Kernel operations.

---

# Part I — Architectural Rules

## 2. Canonical sources of truth

When implementing a client, use these sources in this order:

1. `api/*.fbs` — canonical portable request/response contract.
2. `api/rpc/searchkernel.proto` — transport framing only.
3. `searchkernel-ir` / `*_to_ir()` — canonical portable semantics.
4. `docs/client-api-spec.md` — intended cross-language public facade.
5. `docs/filter-expression-language.md` — filter syntax and clause-local binding semantics.
6. `BUILDING.md`, root `dev`, and `build/targets.json` — canonical repository build/test/verify command surface and ownership mapping.
7. `reference/client-contracts/catalog.json` plus executable `SKC:` annotations — canonical client capability/evidence registry.
8. `reference/client-semantic-parity/`, `reference/result-response-conformance/`, and `reference/client-real-e2e-parity/` — shared cross-language acceptance corpora.
9. Existing first-party clients — implementation references, not independent specifications.
10. Shared conformance/E2E fixtures — executable behavior contract.
11. `clients/INTEGRATING_NEW_CLIENTS.md` — repository integration SOP once the client implementation exists.

The public repository command surface is `./dev`. Direct language-tool,
package-manager, verifier-script, or bookkeeping invocations are implementation
details unless this document explicitly says otherwise.

If an existing client conflicts with the FlatBuffers contract or canonical IR semantics, the existing client is wrong.

If the high-level client specification contains functionality not yet implemented by any first-party client/backend, do not silently invent behavior merely to satisfy the prose. Preserve represented low-level access and implement ergonomic sugar when its behavior is already defined and testable.

---

## 3. What belongs in a language SDK

A language SDK may own:

- idiomatic `Index` facade;
- fluent search/update builders;
- typed query helper syntax;
- grouping/facet/aggregation convenience syntax;
- field/rank-profile convenience constructors;
- result wrappers;
- portable error presentation;
- value conversion between host-language values and the canonical portable `Value` representation;
- connection plumbing for the client's sanctioned transport;
- managed local `searchkerneld` process lifecycle when the client explicitly claims managed-local operation;
- package/install/build integration;
- generated-code orchestration where the selected representation path uses generated artifacts;
- language-native tests and examples.

A language SDK must not own:

- filter parsing;
- filter operator semantics;
- Search Kernel defaults;
- field capability validation;
- schema semantics;
- ranking semantics;
- grouping semantics;
- YQL semantics;
- request normalization;
- backend feature emulation;
- Vespa query lowering;
- a handwritten full-schema serializer/deserializer;
- a protobuf copy of the Search Kernel domain model;
- a language-specific native client ABI.

Client-side validation is limited to host-language misuse and representability checks, for example:

```text
negative limit
a page/offset combination that is locally contradictory
number outside uint32 range
unsafe JS integer
empty required local name
wrong host-language container type
```

Semantic validity belongs on the canonical Rust path.

---

## 4. When used, the gRPC contract is intentionally tiny

The RPC layer is conceptually:

```proto
message FlatBufferFrame {
    bytes payload = 1;
}

service SearchKernel {
    rpc Execute(FlatBufferFrame) returns (FlatBufferFrame);
}
```

Do not add one RPC method per Search Kernel operation.

Do not add protobuf forms of:

```text
SearchRequest
FilterExpr
QueryExpr
GroupingRequest
RankingRequest
Document
Value
Error
```

A new Search Kernel feature should normally require a FlatBuffers change and no `.proto` change.
HTTP-first clients do not use this RPC transport at all; they use the sanctioned
standard-FlatBuffers-JSON path described later.

---

## 5. Complete low-level API + handwritten ergonomic API

Every first-party client has two public conceptual layers.

### 5.1 Low-level portable layer

The low-level layer must expose the complete canonical portable contract for the
client's selected representation path.

For binary/native clients, use standard generated tooling:

```text
flatc       → FlatBuffers models
protobuf/gRPC tooling → transport stubs
```

The generated FlatBuffers object model is the complete low-level portable API.

For an explicitly HTTP-first client, the low-level layer MAY instead expose
standard FlatBuffers JSON envelope/table representations and helpers. In that
case:

- the representation MUST remain standard FlatBuffers JSON;
- the official BFBS-backed parser/printer is the schema authority;
- representative client-emitted requests and values MUST be checked against the
  official BFBS bridge so schema/discriminator drift fails CI;
- the client MUST provide a supported escape hatch for represented capabilities
  that do not yet have ergonomic sugar;
- the client MUST NOT define an independently versioned REST/JSON domain model.

Do not require generated language bindings merely to satisfy a checkbox when the
client never emits binary FlatBuffers. The requirement is canonical contract
coverage and mechanical schema fidelity, not a particular in-language object
model.


### 5.2 Handwritten ergonomic layer

Own only the language experience:

```text
Index
SearchBuilder
YqlBuilder
PatchBuilder
UpdateBuilder
q
Group
g / gf / agg
field definitions
RankProfile
SearchResults
Hit
ResultGroup
```

Do not maintain a handwritten copy of every FlatBuffers table.

---

# Part II — Reference Clients

## 6. Python is the managed-local lifecycle reference

The Python client is the reference implementation for local non-Rust lifecycle behavior.

Its important structure is:

```text
Index.create(...)
        ↓
pack IndexDefinition FlatBuffer
        ↓
_ManagedLocalHost
        ↓
spawn searchkerneld
        ↓
private Unix-domain socket
        ↓
GrpcTransport
        ↓
normal client API
```

`Index.open(...)` uses the same managed host without passing a new definition.

The Python implementation is the behavior reference for:

- `SEARCHKERNEL_LOCAL_HOST`;
- `SEARCHKERNEL_SDK_LIB`;
- local daemon binary resolution;
- `--index`, `--listen`, `--definition` invocation;
- temporary definition file lifecycle;
- private Unix socket lifecycle;
- readiness detection;
- graceful shutdown;
- forced cleanup;
- create/open persistence semantics.

Future native-runtime clients should match this behavior unless the host platform genuinely cannot support it.

---

## 7. TypeScript is the multi-runtime/package-hardening reference

The TypeScript client is the reference for a client that must support both browser and native runtime use.

Its important split is:

```text
@searchkernel/client
    browser-safe facade
    gRPC-Web transport

@searchkernel/client/node
    same facade
    native gRPC transport
    managed searchkerneld lifecycle

@searchkernel/client/low-level
    generated portable model escape hatch
```

Future clients with multiple incompatible runtime environments should use the same principle:

> Keep the portable facade shared. Isolate environment-specific transport/process dependencies behind separate modules or package entrypoints.

Never make browser/mobile builds pull in process-spawning or filesystem dependencies accidentally.

---

# Part III — Implementation Sequence

## 8. Do not start with the ergonomic facade

Implement a new language client in the following order.

Each phase must pass before moving to the next.

```text
0. Inspect/freeze inputs and the current ./dev/build ownership model
1. Generate portable models
2. Generate/implement transport plumbing
3. Implement protocol envelope layer
4. Implement portable values
5. Implement low-level Client
6. Implement result decoding
7. Implement ergonomic facade
8. Implement managed-local lifecycle where applicable
9. Package/export correctly and prove the packaged consumer
10. Port the canonical public-facade E2E
11. Register the client surface, runtimes, evidence roots, and executable E2E lanes
12. Add shared semantic/result conformance
13. Add shared real-engine E2E parity
14. Wire ./dev test/verify/bookkeeping ownership
15. Run focused, repository-wide, and adversarial acceptance
```

Do not combine client creation with changes to Search Kernel semantics, backend lowering, or daemon domain behavior.

### 8.1 The build system is part of the client contract

Normal development and acceptance use the root `./dev` interface. Build/test/verify
commands run in the canonical Search Kernel CI image by default, so a client must
work in that environment rather than depending on packages or state that happen
to exist on a developer host.

Use the smallest relevant command while iterating:

```bash
./dev test --list
./dev verify --list
./dev explain clients/<client>

./dev test <client>
./dev verify contracts
./dev verify client-semantic-parity
./dev test client-real-e2e-parity
```

Use the repository gates in increasing scope:

```bash
./dev check-fast
./dev check
./dev galaxy
./dev universe
```

`./dev ci` is intentionally a cheap mandatory PR gate. A green `./dev ci`
does **not** establish that a new client is complete; client/package/E2E/parity
acceptance lives in the stronger commands above.

When a local-capable developer or agent cannot execute a relevant lane locally,
use the exact-HEAD remote form instead of inventing a one-off workflow:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

Remote verification is evidence for the pushed commit, not for uncommitted local
changes.

The detailed repository-wiring procedure lives in
[`INTEGRATING_NEW_CLIENTS.md`](./INTEGRATING_NEW_CLIENTS.md). This document
defines what must exist; the integration SOP defines every place it must be
registered.

---

# Phase 0 — Inspect and freeze inputs

## 9. Before writing code

An implementation agent must inspect:

```text
api/searchkernel.fbs
api/searchkernel_request.fbs
api/searchkernel_response.fbs
api/rpc/searchkernel.proto
clients/README.md
docs/architecture.md
docs/client-api-spec.md
docs/filter-expression-language.md
```

Also inspect the current Python client and one other first-party facade if available.

Record:

- pinned `flatc` version;
- protocol version;
- request file identifier;
- response file identifier;
- operation request union mappings;
- operation response union mappings;
- canonical error union discriminator;
- supported transport targets;
- package/runtime requirements for the new language.

Do not redesign any of these during client implementation.

---

# Phase 1 — Establish the canonical low-level representation

## 10. Use official FlatBuffers tooling or the official schema-backed JSON boundary

Choose exactly one low-level representation strategy for the client.

### Binary/native clients

Generate language bindings directly from the canonical schemas:

```text
api/*.fbs
    ↓ pinned flatc
language generated model types
```

Requirements:

- use the repository-pinned FlatBuffers compiler version;
- generate object APIs where the language/tooling supports them;
- do not edit generated output manually;
- keep generation deterministic;
- replace the generated output tree atomically or clean it first;
- stale generated files must not survive field/type deletion;
- add a CI/check command that regenerates and fails on diff.

### Explicit HTTP-first clients

Use standard FlatBuffers JSON as the portable low-level representation:

```text
canonical .fbs / BFBS
        ↓
standard FlatBuffers JSON envelope/table shapes
        ↓
generic HTTP execute edge
        ↓
official BFBS-backed parser/printer
```

Requirements:

- do not invent a custom JSON dialect;
- do not create HTTP-specific request/response DTOs on the server;
- expose a supported low-level request/value escape hatch in the package;
- mechanically validate representative emitted envelopes, unions, values, and
  result shapes through the official BFBS-backed bridge;
- make schema/discriminator drift fail CI;
- keep operation mapping centralized and derived from the canonical schema
  vocabulary.

Avoid:

```text
custom .fbs parser
Search-Kernel-specific language generator
independently versioned JSON/REST domain schema
generated-model wrapper DTO for every schema table
handwritten semantic compiler
```

### Representation acceptance

Binary/native clients must fail CI if generated artifacts are stale.

HTTP-first clients must fail CI if their emitted standard FlatBuffers JSON no
longer parses/round-trips through the official BFBS-backed bridge.


---

# Phase 2 — Transport plumbing

## 11. Prefer standard native gRPC by default

For server/runtime languages such as Python, Go, Java, C#, Ruby, Swift, and Node,
binary FlatBuffers over generated gRPC is the default local/native path. A
language explicitly designated as HTTP-first, such as the initial PHP client,
uses the standard FlatBuffers JSON exception in section 13.1 instead:

```text
canonical FlatBuffer bytes
    ↓
FlatBufferFrame.payload
    ↓
normal gRPC unary Execute
```

Prefer generated standard gRPC clients.

Use Unix-domain sockets for managed local operation on supported Unix-like systems. Support TCP where required by the runtime/deployment.

The transport API should be semantically tiny, conceptually:

```text
execute(payload_bytes, deadline/cancellation) -> response_bytes
close()
```

The transport must not know whether the bytes represent Search, Get, Update, or any other Search Kernel operation.

### No client may implement gRPC itself

This is a client-wide architectural rule, not a TypeScript-specific one.

**No Search Kernel client that uses gRPC may implement any part of the gRPC protocol itself.**

Every client that chooses the gRPC transport MUST use the standard client code generated from:

```text
api/rpc/searchkernel.proto
```

using the normal gRPC code generator and runtime for that language.

The required architecture for every gRPC-based non-Rust SDK is:

```text
idiomatic language facade
        ↓
flatc-generated FlatBuffer models
        ↓
canonical RequestEnvelope bytes
        ↓
auto-generated gRPC client stub
        ↓
standard language gRPC runtime
        ↓
searchkerneld
```

Search Kernel client code MUST NOT hand-write or reimplement:

```text
HTTP/2 connection management
gRPC message framing
5-byte frame headers
grpc-timeout encoding
metadata handling
compression handling
status handling
trailers
stream/frame decoding
cancellation protocol
connection lifecycle
gRPC wire-format behavior
```

Those responsibilities belong entirely to the generated gRPC client and its standard runtime.

This applies equally to every client that chooses gRPC, including:

```text
Python
TypeScript / Node.js
Go
Java
C#
Ruby
Swift
future gRPC-based first-party clients
```

An explicitly HTTP-first client is outside this gRPC requirement because it does
not use gRPC. It still MUST NOT hand-write gRPC; it simply uses the sanctioned
HTTP/standard-FlatBuffers-JSON path instead.

A language SDK may wrap the generated client to provide Search Kernel-specific behavior such as:

```text
UDS/TCP endpoint configuration
managed searchkerneld lifecycle
FlatBuffer request construction
FlatBuffer response decoding
portable Search Kernel error mapping
language-idiomatic facade methods
```

But that wrapper must call the generated gRPC stub. It must never replace it.

Therefore, any existing client code that manually implements gRPC framing or HTTP/2 transport — such as the current TypeScript Node implementation — is not something to harden. It should be removed and replaced with the generated client for that language.

Problems such as unsupported compressed frames, assumptions about unary frame layout, trailer handling, or timeout encoding are not bugs Search Kernel should fix individually. Their existence indicates that the client is incorrectly owning protocol machinery that belongs to the gRPC runtime.

The invariant is:

> **Search Kernel owns the payload. The generated gRPC client and standard gRPC runtime own the transport.**

There should be **zero hand-written gRPC protocol implementations anywhere under `clients/`.**

## 12. Browser environments use gRPC-Web

A browser cannot connect to a Unix socket or spawn `searchkerneld`.

Browser path:

```text
browser facade
    ↓
gRPC-Web Execute(FlatBufferFrame)
    ↓
searchkerneld/browser-compatible transport edge
```

Keep browser transport isolated from Node/native runtime dependencies.

## 13. Handwritten transport is an exception

Standard generated gRPC tooling is preferred.

A minimal handwritten transport may be justified when the language/runtime toolchain cannot satisfy a concrete requirement such as:

- UDS support;
- browser packaging constraints;
- unacceptable dependency churn;
- an unavailable standard generated runtime.

If handwritten, it must remain framing-only and must have dedicated wire-level tests.

Do not use a transport limitation as an excuse to create a second Search Kernel API.

### 13.1 Explicit standard FlatBuffers JSON HTTP edge

A client may use the daemon's HTTP/JSON edge when that transport is explicitly
chosen for the client or deployment. This is an alternative representation of
the same portable contract, not permission to define an HTTP-specific Search
Kernel schema.

The required path is:

```text
idiomatic client facade
        ↓
standard FlatBuffers JSON RequestEnvelope
        ↓
generic HTTP execute endpoint
        ↓
official BFBS-backed FlatBuffers JSON parser
        ↓
canonical RequestEnvelope bytes
        ↓
shared dispatcher / *_to_ir()
```

Responses take the inverse path through the official FlatBuffers JSON printer.

An HTTP/JSON client MUST NOT add handwritten server-side request DTOs, REST
operation semantics, defaults, validation, filter parsing, or a custom JSON
dialect. The HTTP endpoint selects transport only; operation identity remains in
the canonical FlatBuffers envelope.

A staged proof client may intentionally implement only a bounded ergonomic
subset while this transport is being proven. A first-class HTTP-first client
does NOT need to add a binary/gRPC path merely to graduate. It must instead
satisfy the applicable gates for:

- complete low-level canonical access;
- BFBS/schema-drift conformance;
- portable values and operation mapping;
- ergonomic/result/error coverage;
- transport correctness;
- packaging;
- E2E and cross-language conformance;
- managed local lifecycle only if that client explicitly claims managed-local
  operation.

---

# Phase 3 — Protocol envelope layer

## 14. Centralize operation mapping

Create one protocol module that owns:

```text
operation name
    ↔ RequestPayload discriminator
    ↔ request low-level representation
    ↔ ResponsePayload discriminator
    ↔ response low-level representation
```

Current portable operations include:

```text
insert
upsert
patch
update
delete
get
search
analyze
describe
inspect_query
```

Do not scatter union tags across builder code.

## 15. Pack requests canonically

Binary/native path:

```text
generated request object
    ↓
RequestEnvelope(protocol_version, discriminator, payload)
    ↓
FlatBuffer root with canonical request file identifier
    ↓
bytes
```

HTTP-first path:

```text
canonical low-level request table/body
    ↓
standard FlatBuffers JSON RequestEnvelope
    ↓
official BFBS-backed parser at the daemon edge
    ↓
canonical FlatBuffer RequestEnvelope bytes
```

The HTTP client MUST NOT invent a second envelope or operation discriminator.

## 16. Unpack responses defensively

Binary/native clients must verify the response file identifier, decode the root,
verify protocol version/discriminator, and reject missing/malformed union
payloads.

HTTP-first clients receive standard FlatBuffers JSON produced by the official
server-side printer. They must verify:

1. protocol version;
2. response discriminator is either the expected operation response or canonical
   Error;
3. the nested payload exists and matches the discriminator;
4. malformed/unknown union states fail closed.

The BFBS-backed server bridge owns the binary FlatBuffer identifier/root check
for the HTTP path. Do not invent an impossible client-side JSON file-identifier
check merely to mirror the binary path.

A malformed response must become a protocol/decoding error, not a Search Kernel
semantic error.

## 17. Make request/response typing operation-specific

In statically typed clients, enforce both sides:

```text
insert -> InsertRequest -> InsertResult
search -> SearchRequest -> SearchResult
...
```

Do not expose a generic API where this typechecks:

```text
execute("insert", SearchRequest(...))
```

even if the response type is constrained.

---

# Phase 4 — Portable values

## 18. Value conversion is a correctness boundary

Host-language values must map losslessly to canonical generated `Value` variants.

Required categories include at least:

```text
null
bool
int64
uint64
float64
string
bytes
list
object/map
tensor
```

Never stringify everything.

Never infer numeric types in ways that change portable meaning.

---

## 19. Numeric rules

### 19.1 int64 / uint64

The portable wire ranges are larger than several languages' ordinary integer types.

A client must:

- preserve the full signed/unsigned 64-bit range;
- reject overflow;
- avoid precision-losing float conversion;
- use native arbitrary-width integers where available;
- otherwise use an explicit lossless representation such as validated decimal strings;
- return a lossless host representation when decoding.

If accepting decimal strings for integer helpers, validate decimal syntax explicitly before parsing.

Do not accidentally accept:

```text
hex literals
empty strings
whitespace-as-zero
floating point notation
```

unless the public API explicitly promises them.

### 19.2 JavaScript-style numeric ambiguity

Languages such as JavaScript have one `number` type, so `129` and `129.0` are indistinguishable.

Do not pretend otherwise.

The TypeScript policy is a useful reference:

```text
safe integral number -> int64
fractional number    -> float64
float64(129)         -> explicit integral-valued float64
bigint               -> signed int64
int64(...) / uint64(...) -> explicit full-range integer
```

Other languages should solve equivalent ambiguities explicitly rather than guess.

### 19.3 Non-finite numbers

Reject non-finite input values unless the canonical `Value` contract explicitly represents that case.

Result relevance is different: Search Kernel results may represent positive or negative infinity. Preserve canonical relevance semantics rather than coercing infinity to null/zero/sentinel values.

---

## 20. Map/object ordering

Canonical producers must emit deterministic semantic-map ordering.

Do not assume the host language's default string ordering matches every other client.

In particular, JavaScript UTF-16 ordering differs from Unicode scalar/code-point ordering for some non-BMP values.

Use a comparator whose ordering matches the canonical cross-language rule used by the protocol/reference clients.

Apply canonical ordering consistently to:

```text
document fields
ValueObject entries
filter bindings
query inputs/properties where represented as maps
rank properties
generated semantic maps
```

---

## 21. Duplicate map keys

Canonical semantic maps must not silently become first-wins or last-wins during decoding.

Reject duplicates deterministically.

Host-language dictionary literals may make duplicates impossible before the SDK sees them; the generated portable representation can still contain duplicates, so the decoder must defend against them.

---

## 22. Bytes stay bytes

Do not silently convert binary values to integer arrays or strings in the public result API.

Use the language's ordinary binary type:

```text
Python bytes
TypeScript Uint8Array
Go []byte
Java byte[] / ByteString wrapper as appropriate
```

This matters for opaque match features and other binary payloads.

---

## 23. Tensor values

The canonical contract may contain multiple tensor payload representations, including dense, sparse, and mixed forms.

A decoder must either:

1. decode a represented tensor payload losslessly; or
2. reject it explicitly as unsupported.

It must never convert an unknown/sparse/mixed tensor into an empty dense tensor.

Unknown future value/tensor discriminators must fail closed.

---

## 24. Missing union payloads fail closed

A discriminator with no corresponding union payload is malformed.

Do not repair malformed values with defaults such as:

```text
missing BoolValue  -> false
missing Int64Value -> 0
missing StringValue -> ""
missing list       -> []
missing object     -> {}
```

Throw a deterministic decoding/protocol error.

---

## 25. Respect narrow wire integers

Where the FlatBuffers field is `uint32`, enforce `0..UINT32_MAX` in local builder validation.

Typical affected fields include:

```text
limit
offset
page-derived offset
facet limits
group limits
group precision
```

Do not validate merely as "non-negative integer" and allow wire truncation/wraparound.

---

# Phase 5 — Low-level Client

## 26. Implement the complete portable operation core first

The low-level client should wrap the canonical request/response representation
for its selected transport path.

Binary/native conceptually:

```text
Client.execute(operation, generated_request)
    ↓
pack RequestEnvelope bytes
    ↓
transport.execute(bytes)
    ↓
unpack ResponseEnvelope
    ↓
canonical Error? -> SearchKernelError
    ↓
generated response object
```

HTTP-first conceptually:

```text
Client.execute(operation, canonical_json_request)
    ↓
standard FlatBuffers JSON RequestEnvelope
    ↓
HTTP transport
    ↓
standard FlatBuffers JSON ResponseEnvelope
    ↓
canonical Error? -> SearchKernelError
    ↓
low-level response representation
```

Then expose operation methods:

```text
insert
upsert
patch
update
delete
get
search
analyze
describe
inspect_query
```

This client is the foundation for the ergonomic `Index` facade.

---

## 27. Keep three error categories distinct

### Transport error

Examples:

```text
connection refused
deadline exceeded at transport layer
cancelled stream
malformed gRPC frame
closed transport
```

### Protocol/decoding error

Examples:

```text
wrong FlatBuffer root/file identifier
unsupported protocol version
wrong response union discriminator
missing nested union payload
malformed generated object
```

### Search Kernel portable error

A valid `ResponseEnvelope` whose payload is canonical `Error`.

Expose stable fields:

```text
code
message
details
retryable
```

Do not collapse numeric error enums into opaque numbers when stable symbolic names are available.

---

# Phase 6 — Result model

## 28. Preserve the recursive canonical tree

The canonical result is recursive:

```text
SearchResult
    root: ResultNode
        Hit
        ResultGroup
            children: ResultNode[]
```

The SDK must preserve this shape.

Convenience views such as:

```text
hits
groups
facets
aggregations
```

must be non-destructive projections over the canonical tree.

Do not flatten grouped hits into top-level hits.

Do not silently drop unknown/malformed result children.

---

## 29. Hit wrapper requirements

Expose the canonical result information available from the wire, including where supported:

```text
id
relevance / score
fields
highlights
match features
summary features
rank features
sort values
source/types/metadata where represented
```

Field convenience access is encouraged where idiomatic, but it must not hide metadata.

---

## 30. ResultGroup wrapper requirements

Expose:

```text
value
label
fields
children
groups
hits
continuations
```

Preserve child order.

Continuation tokens are opaque strings. Never interpret or rewrite them in the SDK.

---

## 31. Coverage and result errors

Preserve structured coverage and degradation fields.

Do not collapse rich coverage data into only a percentage.

Search-result embedded errors are distinct from operation-level `SearchKernelError` and should remain available on the result object.

---

# Phase 7 — Ergonomic facade

## 32. Port behavior, not syntax character-for-character

The public facade should feel idiomatic in the target language while preserving the same concepts:

```text
Index
SearchBuilder
YqlBuilder
PatchBuilder
UpdateBuilder
q
Group
g
gf
agg
field definitions
RankProfile
SearchResults
```

Use the cross-language client specification for vocabulary.

Use current first-party client behavior and shared fixtures to determine what is actually implemented and expected today.

---

## 33. `Index`

Remote/client mode should support the language equivalent of:

```text
Index.connect(endpoint)
```

Clients that explicitly claim managed-local operation should additionally support managed local lifecycle as described later. Runtime capability alone does not force a connect-only client to claim daemon ownership.

Ordinary complete commands execute directly:

```text
insert
upsert
get
delete
describe
```

Configurable operations use builders:

```text
search
browse
query
yql
patch
update
```

---

## 34. Filter strings are transported, not parsed

The ergonomic API should support:

```text
filter(expression)
filter(expression, bindings)
```

The SDK serializes:

```text
FilterClause {
    source
    bindings
}
```

It does not implement:

```text
AND/OR/NOT precedence
field lookup
type checking
IN semantics
missing-field semantics
```

Bindings are clause-local.

Repeated filters remain distinct clauses. Do not concatenate their source strings.

---

## 35. Query helpers

Typed query helpers should construct the client's canonical low-level query representation immediately.

Do not invent a parallel handwritten AST that later needs a second semantic compiler unless the host language requires a tiny ergonomic wrapper.

Representative helpers include:

```text
term
contains
phrase
near / ordered-near
same-element
fuzzy
AND / OR / NOT
weak-and
WAND
rank
geo
nearest-neighbor
```

The exact ergonomic surface should follow the current cross-language contract and implemented client parity.

---

## 36. Grouping/facets/aggregations

`facet()`, `aggregate()`, and `Group.by()` are convenience constructors over canonical grouping.

Do not create a separate facet engine/model.

The facade should preserve:

```text
nested groups
limits
precision
ordering
outputs
aliases
keep/filter clauses
summary aggregations when supported
continuations
```

Grouping result conveniences must derive from the exact result tree generated by those request shapes.

---

## 37. YQL remains an explicit escape hatch

YQL is not the filter language and not the typed query DSL.

Transport it through the canonical YQL query source representation.

Keep query properties distinct from:

```text
filter-local bindings
query inputs
rank properties
```

Do not parse YQL client-side to guess or merge semantics.

---

## 38. Patch and update are intentionally separate

Keep the simple partial update facade separate from the advanced update model.

Typical patch surface:

```text
set
unset
```

Typical update surface:

```text
assign/set
clear/unset
increment
decrement
multiply
divide
condition
create
```

Additional portable update operations may be added ergonomically as implemented, but the supported low-level canonical representation remains available immediately.

---

# Part IV — Managed Local `searchkerneld`

## 39. Which clients should support managed local lifecycle

Managed local lifecycle is a capability claim, not a universal requirement.

A first-party client MUST implement `Index.create()` / `Index.open()` and own
`searchkerneld` only if that client/package explicitly claims managed-local
operation.

A connect-only client can still be first-class when its intended deployment
model is an already-managed daemon, including shared-host/server deployments.
Such a client must:

- document that lifecycle boundary clearly;
- expose `Index.connect(endpoint)` or its language equivalent;
- not claim managed-local capability evidence;
- still verify persistence/reopen semantics through an external harness when
  those semantics are part of its public behavior.

If a client does claim managed-local operation and the runtime can:

- spawn child processes;
- access the filesystem;
- connect to the required local endpoint;
- perform deterministic cleanup;

then it must satisfy the remaining managed-host requirements in this part.

Browsers and other connect-only environments remain `Index.connect(...)` only.

---

## 40. Managed lifecycle must mirror Python

The required conceptual implementation is:

```text
Index.create(path, schema/rank profiles)
        ↓
build generated IndexDefinition
        ↓
pack IndexDefinition FlatBuffer root
        ↓
ManagedLocalHost(path, definition_bytes)
        ↓
spawn searchkerneld
        ↓
connect normal client to private endpoint
```

For reopen:

```text
Index.open(path)
        ↓
ManagedLocalHost(path, no definition)
        ↓
spawn searchkerneld against existing index
```

Do not create a new JSON schema/config format for startup.

---

## 41. Host binary resolution

Use the same convention across clients where practical:

1. `SEARCHKERNEL_LOCAL_HOST` environment override;
2. package-bundled `bin/searchkerneld` if the package strategy ships one;
3. repository development fallback such as the active target's
   `target/<environment>/debug/searchkerneld`.

If the binary is absent, fail with a clear client error explaining that `searchkerneld` must be built/installed.

---

## 42. Native SDK library resolution

On development Linux setups, match Python behavior:

1. use `SEARCHKERNEL_SDK_LIB` if configured;
2. otherwise detect the selected environment's native SDK lib directory
   (`SK_NATIVE_SDK` or the active target's `native-sdk/linux-x86_64`); see
   [`docs/native-build-provenance.md`](../docs/native-build-provenance.md);
3. prepend it to `LD_LIBRARY_PATH` for the child only.

Do not mutate the parent process environment globally.

Platform-specific packaging may replace this mechanism later, but local development behavior should remain deterministic.

---

## 43. Private socket and definition resources

For create:

- create a private temporary directory;
- place the Unix socket inside it;
- write the packed `IndexDefinition` bytes to a temporary `.fb` file;
- remove both during cleanup.

For open:

- create only the private socket resources;
- do not pass a definition.

The endpoint should conceptually be:

```text
unix:///tmp/.../searchkerneld.sock
```

---

## 44. Daemon invocation

Match the established CLI contract:

```text
searchkerneld
    --index <index path>
    --listen <private endpoint>
    [--definition <packed definition file>]
```

Do not add language-specific daemon flags.

---

## 45. Process ownership boundary

Start the daemon in a private process group/session where the host runtime supports it.

The client owns the daemon and its children.

Explicit close must terminate the full owned group, not leave orphan helpers behind.

Install only best-effort process-exit cleanup as a last resort. Normal cleanup must happen through explicit `close`/dispose/context-manager paths.

---

## 46. Readiness

Do not sleep for an arbitrary fixed duration.

Use the Python behavior:

```text
for up to ~15 seconds:
    if child exited -> startup failure
    attempt connection to private socket
    if connected -> ready
    short delay and retry
```

Readiness here proves that the transport endpoint exists. A later protocol smoke test may additionally call a lightweight operation.

---

## 47. Shutdown

Normal close sequence should mirror Python:

```text
send graceful interrupt (SIGINT on POSIX)
wait up to ~15 seconds
if still alive:
    SIGKILL
    wait up to ~5 seconds
close transport/resources
remove temporary files/directories
```

Force-cleanup during failed startup may go directly to hard termination.

Close must be idempotent.

---

## 48. Keep managed hosting separate from semantics

Managed hosting owns only:

```text
process
filesystem temp resources
socket endpoint
startup/readiness
shutdown
```

Once the daemon is ready, all requests go through the ordinary client/protocol/transport stack.

Never add local-only Search Kernel semantics to `Index.create/open`.

---

# Part V — Multi-Runtime Package Design

## 49. Separate incompatible runtime dependencies

If a language ecosystem has browser/mobile/server variants, keep environment-specific code separate.

TypeScript reference:

```text
main entrypoint
    browser-safe API + gRPC-Web

/node
    Node native gRPC + process spawning

/low-level
    generated portable models
```

Equivalent separation should be used elsewhere where useful.

A browser consumer must not accidentally pull in:

```text
child_process
filesystem
Unix socket APIs
native gRPC runtime
```

---

## 50. Packaging must be tested from the published artifact

Do not test only source-tree imports.

Acceptance must build/package exactly what users install, then consume that artifact from a small external fixture.

Verify:

- declared main/module entrypoints exist;
- supported low-level canonical export works;
- runtime-specific entrypoint works;
- type declarations/metadata are present;
- examples intended for distribution are actually included;
- browser package builds in a browser bundler if browser support is claimed.

For npm-like ecosystems, ensure a clean checkout can `pack/publish` without silently omitting compiled output. Use a build/prepack hook where needed.

---

# Part VI — Generation and Repository Hygiene

## 51. Generated output rules

Generated code should live in clearly marked generated directories.

Requirements:

- no manual modifications;
- deterministic regeneration;
- pinned generators;
- no orphan generated files;
- package imports should not depend on accidental internal generator layout unless exported intentionally.

Standard tooling owns standard generation.

---

## 52. Avoid unnecessary repository churn

When adding a new client:

- do not refactor unrelated clients;
- do not rename canonical schemas;
- do not reformat large generated trees unnecessarily;
- do not change `searchkerneld` semantics to suit one language;
- do not change backend code unless a genuine protocol/backend defect is proven;
- prefer additive files and small shared hooks.

When porting from an existing first-party client, preserve behavior while adapting syntax idiomatically.

---

# Part VII — Testing Strategy

## 53. A client is not complete because it serializes

Each first-party SDK needs several independent test layers. They prove different
claims and must not be collapsed into one "client tests passed" signal.

```text
A. low-level representation/reproducibility checks
B. protocol tests
C. value conversion tests
D. low-level client tests
E. facade/builder tests
F. result decoding tests
G. transport tests
H. managed-host tests where applicable
I. packaged-artifact consumer tests
J. registered real-product E2E lanes
K. canonical request/IR semantic conformance
L. frozen result-response conformance
M. shared real-engine cross-client E2E parity
```

The evidence model is deliberately split:

```text
ergonomic    the handwritten facade exposes the capability
conformance  shared canonical fixtures prove equivalent meaning/decoding
e2e          a registered lane reaches the real daemon/backend path
```

Those are independent. A builder test cannot prove real-runtime behavior. A
low-level E2E cannot prove the ergonomic facade exists. A comment in a test file
cannot prove E2E unless the repository knows how to execute that exact
file/runtime.

For normal iteration, the language's complete ordinary suite is exposed as:

```bash
./dev test <client>
```

That target should own the client's language tests, package/consumer proof, and
registered surface E2E as applicable. Do not make developers reconstruct the
client's test suite from raw package-manager commands.

---

## 54. Protocol tests

Cover at least:

```text
protocol version
all operation discriminators
expected response discriminator
canonical Error discriminator
wrong discriminator rejection
truncated/malformed response
malformed nested union payload
unknown operation
```

Binary clients must additionally verify request/response file identifiers and
root decoding. HTTP-first clients must instead prove that representative
client-emitted standard FlatBuffers JSON parses/round-trips through the official
BFBS-backed bridge and that malformed JSON/envelope shapes fail closed.

Do not write a test named "has SKRQ" that merely checks the result is a byte
array, and do not treat "JSON parsed" as sufficient HTTP conformance. Inspect the
actual canonical contract.

---

## 55. Value tests

Cover at least:

```text
null
true/false
int64 min/max
uint64 min/max
out-of-range integers
invalid decimal strings
float64 fractional
integral-valued float64 ambiguity where relevant
non-finite input rejection
bytes
nested list/object
canonical map ordering including non-BMP Unicode
map duplicate rejection
empty/missing union payload rejection
dense tensor
sparse tensor
mixed tensor
unknown future discriminator fails closed
uint32 max/max+1 for relevant builder fields
```

---

## 56. Compile/type tests

For statically typed languages, include compile-positive and compile-negative fixtures.

Examples:

```text
insert accepts InsertRequest
insert rejects SearchRequest
search returns SearchResult
supported low-level canonical API is importable
builder method combinations typecheck as intended
```

Runtime tests do not replace compile-contract tests.

---

## 57. Facade tests

Test construction of canonical request meaning for:

```text
search/browse/query/yql
filter clause-local bindings
repeated filters
sort
fields projection
page/offset/limit
ranking profile
query inputs
rank properties
properties
facets
grouping
aggregations
patch
advanced update
```

When possible, compare resolved/recorded semantics rather than exact FlatBuffer byte identity.

Equivalent requests do not need byte-for-byte identical serialization unless the project explicitly promises canonical byte encoding.

---

## 58. Result tests

Freeze recursive fixtures containing:

```text
root hits
single group with hits
nested groups
mixed hit/group children
facet-shaped groups
root aggregations
continuations
coverage/degradation
finite relevance
positive/negative infinite relevance
features
highlights
sort values
embedded result errors
```

Malformed result nodes must fail closed rather than disappear silently.

---

## 59. Transport tests

For each supported transport:

- closed/unavailable transport;
- cancellation where the language/transport supports it;
- deadline/timeout behavior where claimed;
- message size limits;
- malformed transport response/frame;
- successful request/response;
- endpoint connectivity for each claimed endpoint type.

gRPC transports must additionally cover non-zero gRPC status, generated runtime
behavior, and UDS/TCP connectivity as claimed.

HTTP-first transports must additionally cover HTTP method/content-type/status
handling, malformed JSON responses, redirects/proxy status chains where
supported, and the generic execute endpoint.

Browser gRPC-Web clients must include a build/runtime smoke test if browser
support is claimed.

Transport tests must not depend on Search Kernel semantics beyond carrying the
canonical payload representation.

---

## 60. Managed-host tests

For clients that claim managed-local operation, test with a controllable fake daemon before relying only on the full engine.

Verify:

```text
exact CLI args
SEARCHKERNEL_LOCAL_HOST override
private socket allocation
packed definition file exists for create
no definition for open
readiness detects socket
startup child death is reported
SIGINT graceful close
SIGKILL fallback
idempotent close
temporary resources removed
process group cleanup
```

Then verify against real `searchkerneld` in E2E.

---

## 61. Package tests

Package from a clean tree.

Install/consume the resulting artifact in a separate fixture.

Verify every supported public entrypoint and, where relevant:

```text
CommonJS/ESM import behavior
browser bundler build
native-runtime import
low-level canonical import
type declarations
example compilation
```

Do not declare a package publishable if the packed artifact is missing the compiled files referenced by its manifest.

---

# Part VIII — Canonical E2E

## 62. Every first-class client needs a real user-facing E2E example

Create a language-equivalent of:

```text
try_searchkernel_client_api_e2e
```

It serves simultaneously as:

- executable documentation;
- public API showcase;
- real-engine acceptance test;
- parity reference for future clients.

For clients that claim managed-local operation, the example should own its temporary index and daemon just like Python.

For connect-only clients, including HTTP-first server clients and browser-only clients, a separate harness may prepare the daemon/index. The language E2E must still exercise the public package facade rather than bypassing through generated/protocol internals.

An E2E file is not accepted merely because it exists or contains an `SKC:`
annotation. Register the exact executable lane in
`reference/client-contracts/catalog.json` with its runtime(s), path, and
repository-root command. If the runtime is required for the production surface,
list it in `required_e2e_runtimes` so omission is a registry failure.

The normal execution path is:

```text
SKC kinds=e2e annotation
        ↓
catalog e2e_lanes entry
        ↓
./dev test <client>
        ↓
tools/check-client-contracts.py --run-e2e-surface <client>
        ↓
real searchkerneld / backend
```

The internal checker invocation is shown only to explain the chain; users and
agents should normally run the `./dev` target.

---

## 63. E2E scenario coverage

The current canonical showcase should cover as much of the implemented public surface as the backend supports.

### Lifecycle/schema

```text
create fresh index
field definitions
rank profiles
close
reopen same persistent index
```

### Documents

```text
insert deterministic corpus
get
upsert replacement
delete + duplicate delete
patch set/unset
advanced update set/unset/arithmetic/create
persistence of mutations across reopen
```

### Search/filtering

```text
full-text search
stemming/linguistics representative
string filters
clause-local bindings
same placeholder reused across clauses with different values/types
repeated filters => server-side AND semantics
OR / NOT / IN / parentheses
sorting
projection
offset/limit
page convenience
builder misuse rejection
total hits
structured filter errors
```

### Facets/grouping/aggregation

```text
facet convenience
filtered facets
root count/sum/avg/min/max/stddev where supported
single-level group
nested group
group limit/precision
aggregation ordering
aliases/output
group keep/filter
recursive result decoding
summary aggregation when backend support exists
continuations when backend support exists
```

### Query/ranking/vector

```text
typed query representative
query input namespace
rank property namespace
ranking profile
scalar match features
tensor query input
nearest-neighbor
hybrid lexical + vector candidate union
rank score consistency where deterministic
```

### YQL

```text
raw YQL
@property substitution
pagination
ranking
programmatic grouping
```

### Results

```text
field access
dense tensor readback
opaque binary features
coverage/degradation
recursive groups
facets/aggregations convenience projections
```

If a feature is blocked by an explicitly known backend limitation, leave the E2E case documented as blocked/deferred rather than deleting all trace of it.

---

## 64. Port the E2E semantically

Do not blindly transliterate syntax.

Examples:

- asynchronous languages must `await` operations;
- JavaScript needs explicit `float64(129)` where Python `129.0` carries type information;
- int64 result assertions may need `bigint`;
- host-language binary types differ;
- context managers become `defer`, `try/finally`, `Dispose`, `AutoCloseable`, etc.

The semantic scenario and expected outcome should remain equivalent.

---

# Part IX — Cross-Language Conformance

## 65. Semantic parity is the acceptance target

For representative calls:

```text
Python facade
TypeScript facade
Go facade
Java facade
...
        ↓
canonical portable request
        ↓
same *_to_ir()
        ↓
equivalent IR meaning
```

Likewise, canonical scripted responses/errors should produce equivalent language-level meaning.

Do not compare only pretty-printed facade objects.

Do not require byte-for-byte identical FlatBuffers unless explicitly specified.

---

## 65.1 Record executable capability evidence

When a client test proves a canonical capability, add an `SKC:` annotation
beside that test or E2E scenario. Positive client support in
[`docs/CLIENT_CAPABILITIES.md`](../docs/CLIENT_CAPABILITIES.md) is generated
from those annotations; do not maintain a separate compatibility boolean.

Use independent evidence kinds:

```text
ergonomic    handwritten high-level facade exposes it
conformance  shared semantic/contract fixture proves parity
e2e          real product/backend path executes it
```

A test may prove more than one kind, but do not infer `ergonomic` merely
because a low-level E2E reaches the backend. Record the runtime explicitly.

The catalog is
[`reference/client-contracts/catalog.json`](../reference/client-contracts/catalog.json).
For a new first-class surface, register:

```text
runtimes
evidence_roots
required_e2e_runtimes where production support requires runtime proof
e2e_lanes with exact path + command
parity_requirements only when an explicit source→target parity contract is intended
```

Use repository bookkeeping, not the checker directly:

```bash
./dev bookkeeping sync
./dev bookkeeping check
./dev verify contracts
```

`bookkeeping sync` regenerates deterministic reports such as
`docs/CLIENT_CAPABILITIES.md` and `docs/client-capabilities.json`; the checked
state must be clean afterward. Do not hand-edit generated compatibility output
and do not maintain parallel support booleans.

### 65.2 Cross-language compatibility has separate executable gates

A first-class client participates in three stronger parity mechanisms in addition
to its own `./dev test <client>` suite:

1. **Canonical semantic parity**
   ```bash
   ./dev verify client-semantic-parity
   ```
   This executes the frozen cross-language request/IR corpus and the frozen
   result-response schema/conformance corpus. Adding a client means extending
   those harnesses so the new language consumes the same inputs and is compared
   under the same rules; merely having equivalent-looking unit tests is not
   enough.

2. **Result-response conformance**
   The client must decode the shared encoded response fixtures under
   `reference/result-response-conformance/`, including malformed cases and
   recursive result shapes. Do not regenerate fixtures to make a new client
   pass. `./dev regen result-fixtures` is an authoring command for intentional
   canonical fixture changes.

3. **Real-engine E2E parity**
   ```bash
   ./dev test client-real-e2e-parity
   ```
   This runs each first-party showcase against the real engine using the shared
   scenario intent in `reference/client-real-e2e-parity/corpus.json`, captures
   normalized observations, and compares them across clients. A new first-class
   client must be added to the comparator and its shared scenario
   classifications; it must not carry a private "equivalent" corpus.

These gates catch different regressions. Keep all three.

### 65.3 Compatibility evidence should be executable and omission-sensitive

The target state is not "the new client has tests." It is:

> If the client disappears from generation, capability evidence, result
> conformance, semantic parity, packaging, or real E2E, a normal repository
> command fails.

That is why compatibility metadata is derived from executable evidence and why
required E2E runtimes are explicit.

## 66. Parity checklist

For every first-class client, verify parity for:

```text
operation coverage
filter clause locality
portable values
numeric ranges
query sources
grouping/facets
ranking inputs/properties
result tree
errors
managed lifecycle where claimed
persistent reopen behavior where applicable
```

An ergonomic helper may lag behind another client only if the supported low-level canonical representation remains available and the lag is documented.

---

# Part X — Common Failure Modes

## 67. Do not repeat these mistakes

### Failure: building only a transport skeleton

A client is not complete because it can call `Execute()`.

Port the actual ergonomic surface and result model expected of first-party clients.

### Failure: hiding the low-level portable API

If package exports make the canonical low-level representation impossible to
reach, represented-but-not-ergonomic functionality becomes unusable.

Binary clients should expose their generated portable types. HTTP-first clients
should expose their supported standard-FlatBuffers-JSON low-level
envelope/value/request helpers.

Provide a supported low-level entrypoint.

### Failure: copying Python's numeric assumptions into JavaScript

Python distinguishes `int` and `float`; JavaScript does not.

Resolve host-language differences explicitly.

### Failure: generic union decoding with defaults

Do not use patterns like:

```text
payload?.value ?? 0
```

for discriminated portable unions.

Switch on the discriminator and require the matching payload.

### Failure: silently losing sparse/mixed tensor data

Decode losslessly or fail.

### Failure: assuming host map ordering is canonical

Test non-ASCII and non-BMP keys.

### Failure: checking only response typing

Request type must also be bound to the operation discriminator.

### Failure: publishing source without built entrypoints

Package from a clean tree and consume the packed artifact.

### Failure: claiming browser support after only Node tests

Build a real browser consumer fixture.

### Failure: concatenating filter strings

Repeated `.filter(...)` calls are separate clauses with separate binding scopes.

### Failure: client-side semantic parsing

Do not implement the Search Kernel filter/query/grouping semantics in every SDK.

### Failure: introducing JSON because a client library finds it convenient

JSON is not a second domain contract.

### Failure: spawning `searchkerneld` with language-specific startup semantics

Match the established managed-host contract.

### Failure: fixed startup sleeps

Probe readiness deterministically.

### Failure: orphan daemon processes

Own the process group/session and implement hard cleanup fallback.

### Failure: treating mandatory PR CI as client acceptance

`./dev ci` is intentionally cheap. Run the client, compatibility, parity, and
real-engine gates that establish the claims your change makes.

### Failure: adding an `SKC:` E2E annotation without an executable lane

Capability comments are evidence pointers, not magic. Register the exact
path/runtime/command in the client-contract catalog and execute it through the
canonical client test surface.

### Failure: running private language commands as the documented acceptance path

Raw `cargo`, `npm`, `pip`, `gem`, `php`, `ruby`, and internal
`tools/check-*.py` commands may be useful while debugging, but first-class
repository verification must be reachable through `./dev`.

### Failure: creating a client-specific parity corpus

Reuse the frozen semantic, result-response, and real-engine scenario corpora.
Host-language adaptations belong in the client adapter, not in a forked truth
set.

### Failure: unnecessary refactors while adding a client

Make the smallest additive repository diff necessary.

---

# Part XI — Agent Workflow

## 68. Recommended execution workflow for an implementation agent

### Step 1 — Read the build and protocol authorities first

Read:

```text
BUILDING.md
clients/README.md
clients/INTEGRATING_NEW_CLIENTS.md
docs/architecture.md
docs/client-api-spec.md
docs/filter-expression-language.md
api/*.fbs
api/rpc/searchkernel.proto
reference/client-contracts/README.md
reference/client-contracts/catalog.json
reference/client-semantic-parity/
reference/result-response-conformance/
reference/client-real-e2e-parity/
```

Then inspect the current Python client and at least one client with a similar
transport/runtime model.

On an unfamiliar checkout, run `./dev doctor` before diagnosing build failures
as product-code failures.

### Step 2 — Write an implementation and evidence inventory

Before coding, list:

```text
low-level representation strategy (generated binary models or schema-backed JSON)
gRPC generation strategy if the client uses gRPC
package/module layout and package-manager artifact
low-level escape hatch
value conversion and numeric-width policy
result wrappers
managed-local capability
browser/mobile constraints
catalog runtimes and evidence roots
required E2E runtimes
public E2E lane(s)
semantic-parity adapter
result-response-conformance adapter
real-engine-parity adapter
./dev test target
package-consumer test
```

Resolve host-language representability differences explicitly. Decide which
claims are ergonomic, conformance, and E2E claims before annotating tests.

### Step 3 — Implement the low-level proof first

Prove the complete low-level path before large ergonomic builder work.

Binary/native example:

```text
generated request object
    ↓
RequestEnvelope bytes
    ↓
generated gRPC transport
    ↓
ResponseEnvelope bytes
    ↓
generated result object
```

HTTP-first example:

```text
canonical low-level request
    ↓
standard FlatBuffers JSON RequestEnvelope
    ↓
HTTP execute edge / BFBS parser
    ↓
standard FlatBuffers JSON ResponseEnvelope
    ↓
low-level response representation
```

### Step 4 — Port the facade incrementally

Recommended order:

```text
values/errors
Index CRUD
search/browse
filters/sort/pagination/projection
result wrappers
patch/update
query helpers
ranking inputs/properties
facets/grouping/aggregations
YQL
schema/rank-profile helpers where applicable
managed lifecycle if claimed
```

Port executable tests with each slice. Add `SKC:` annotations only where the
test actually proves the named capability and evidence kind.

### Step 5 — Prove the package artifact, not only the source tree

Build the package users will install, consume it from a clean external fixture,
and run the public entrypoints from that installed artifact. This proof belongs
inside the future `./dev test <client>` target.

Do not let source-tree imports, workspace-relative generated files, or a
developer's globally installed dependencies hide a broken package.

### Step 6 — Port and register the canonical public E2E

Port `try_searchkernel_client_api_e2e` semantically. It must use the public
package/facade that users are expected to use.

Register every claimed E2E runtime in
`reference/client-contracts/catalog.json`. Add `required_e2e_runtimes` for
production runtime claims that must never lose executable backend coverage.

At this point the client-specific command should be real:

```bash
./dev test <client>
```

### Step 7 — Join the shared compatibility harnesses

Add the client to:

```text
canonical semantic parity
frozen result-response conformance
shared real-engine E2E parity
```

Use the existing shared corpora; do not fork expected behavior per language.

Run:

```bash
./dev verify client-semantic-parity
./dev test client-real-e2e-parity
```

### Step 8 — Register build ownership and deterministic bookkeeping

Wire the new surface into the canonical repository command/ownership model as
described in `INTEGRATING_NEW_CLIENTS.md`. Use `build/targets.json` and
`./dev explain <path>` so future agents can discover the focused and strong
checks for the client.

Then run:

```bash
./dev bookkeeping sync
./dev bookkeeping check
./dev verify contracts
```

The sync must not leave unexplained generated drift.

### Step 9 — Run increasing-scope repository gates

During implementation, prefer focused commands. Before calling the client
first-class, run:

```bash
./dev check-fast
./dev test <client>
./dev verify contracts
./dev verify client-semantic-parity
./dev test client-real-e2e-parity
./dev check
```

For final exhaustive qualification of the integrated client, run:

```bash
./dev universe
```

If a relevant test/verifier cannot run locally, push the exact commit and use
the supported `--remote` form. Do not substitute "CI is green" for a missing
strong lane.

### Step 10 — Independent adversarial review

The reviewer should specifically try to break:

```text
numeric boundaries
union discriminators
malformed response handling
unknown future enum values
map ordering/duplicates
result-tree preservation
package exports and clean installed-artifact consumption
browser/native split
transport cancellation/deadlines
managed daemon cleanup
SKC evidence strength/runtime correctness
unregistered or falsely-green E2E lanes
canonical semantic parity
result-response conformance
real-engine scenario parity
build/target omission
```

Fix findings without broad unrelated refactoring.

---

# Part XII — Definition of Done

## 69. A new client is first-class only when all applicable gates pass

### Contract/generation

- [ ] Low-level portable representation is derived from canonical `.fbs`.
- [ ] Binary clients use generated FlatBuffers models from the pinned toolchain.
- [ ] gRPC clients use generated transport from the canonical `.proto`.
- [ ] Explicit HTTP-first clients use only standard FlatBuffers JSON through the official BFBS-backed bridge.
- [ ] HTTP-first client-emitted representative envelopes/values/unions are schema-drift checked against the official BFBS bridge.
- [ ] Pinned tool/bridge versions are enforced where applicable.
- [ ] Regeneration is deterministic where generated artifacts exist.
- [ ] Stale generated files cannot survive regeneration where generated artifacts exist.
- [ ] Supported low-level canonical API is publicly reachable.

### Protocol

- [ ] Binary clients verify correct request/response identifiers; HTTP-first clients prove canonical roots through BFBS bridge conformance.
- [ ] Correct protocol version handling.
- [ ] Every operation has correct request and response union mapping.
- [ ] Canonical Error discriminator is handled.
- [ ] Malformed/wrong envelopes fail closed.
- [ ] Request types are statically tied to operations where possible.

### Values

- [ ] int64/uint64 are lossless across full range.
- [ ] float64 typing is unambiguous for the host language.
- [ ] non-finite inputs handled according to contract.
- [ ] bytes stay binary.
- [ ] null/bool/string/list/object are lossless.
- [ ] tensors are lossless for supported payloads.
- [ ] unknown/malformed unions fail closed.
- [ ] duplicate semantic map keys are rejected.
- [ ] canonical map ordering matches reference behavior.
- [ ] uint32-backed builder fields are range checked.

### Ergonomics

- [ ] `Index` CRUD parity.
- [ ] Search/browse/query/YQL entrypoints.
- [ ] Filter clause-local bindings.
- [ ] Sort/projection/pagination.
- [ ] Patch/update.
- [ ] Query helpers.
- [ ] Ranking/input/property namespaces.
- [ ] Facets/grouping/aggregations.
- [ ] Field/rank-profile constructors as applicable.
- [ ] Public result wrappers preserve canonical tree.

### Errors/results

- [ ] Transport errors are distinct from portable errors.
- [ ] Protocol errors are distinct from portable errors.
- [ ] Stable portable error codes/details/retryable are exposed.
- [ ] Result errors remain structured.
- [ ] Coverage/degradation retained.
- [ ] Infinite relevance retained.
- [ ] Binary/opaque features retained.

### Transport

- [ ] At least one explicitly sanctioned transport is complete for the client.
- [ ] Default native/runtime clients use generated gRPC carrying canonical FlatBuffer bytes.
- [ ] Explicit HTTP-first clients use only standard FlatBuffers JSON through the BFBS-backed generic execute edge.
- [ ] UDS works where local management is supported and claimed.
- [ ] TCP/HTTP endpoint connectivity works where claimed.
- [ ] gRPC-Web works if browser support is claimed.
- [ ] deadlines/cancellation behave correctly for the selected transport.
- [ ] message size limits are enforced.

### Managed local lifecycle

These gates apply only to clients/packages that claim managed-local operation:

- [ ] `Index.create()`.
- [ ] `Index.open()`.
- [ ] Same daemon CLI contract as Python.
- [ ] Same environment override conventions.
- [ ] Private local endpoint.
- [ ] Deterministic readiness.
- [ ] graceful shutdown + hard fallback.
- [ ] process group/session ownership.
- [ ] temporary resource cleanup.
- [ ] persistence verified across close/reopen.

Connect-only clients are not blocked by these gates, but MUST document that
lifecycle boundary and MUST NOT claim managed-local capability support.

### Packaging

- [ ] Clean build from checkout.
- [ ] Clean package/publish artifact contains declared entrypoints.
- [ ] External consumer can import high-level API.
- [ ] External consumer can import the supported low-level canonical API.
- [ ] Runtime-specific entrypoints work.
- [ ] Browser bundle succeeds if browser support is claimed.
- [ ] Examples intended for users are included and compile.

### Repository/build-system integration

- [ ] `reference/client-contracts/catalog.json` contains the client surface, real runtimes, and evidence roots.
- [ ] Every claimed E2E file/runtime is backed by an exact `e2e_lanes` entry.
- [ ] Production runtimes that require backend proof are listed in `required_e2e_runtimes`.
- [ ] `SKC:` annotations use the correct independent evidence kinds and runtime.
- [ ] `./dev test <client>` is registered and owns the complete ordinary client/package/E2E suite.
- [ ] `build/targets.json` maps the client area to appropriate focused and strong checks.
- [ ] `./dev explain clients/<client>` exposes discoverable ownership/check guidance.
- [ ] `./dev bookkeeping sync` followed by `./dev bookkeeping check` is clean.
- [ ] `./dev verify contracts` passes.
- [ ] Generated compatibility reports are current; no hand-maintained positive support matrix was added.

### Tests/E2E/parity

- [ ] Protocol tests.
- [ ] Value boundary tests.
- [ ] Compile/type contract tests where applicable.
- [ ] Facade builder tests.
- [ ] Result fixture tests.
- [ ] Transport tests.
- [ ] Managed-host tests where managed-local operation is claimed.
- [ ] Packaged-artifact consumer tests.
- [ ] Full public API E2E against real `searchkerneld`/Vespa.
- [ ] Close/reopen persistence E2E where applicable; connect-only clients may use an external harness.
- [ ] Client participates in `./dev verify client-semantic-parity`.
- [ ] Client consumes the frozen `reference/result-response-conformance/` fixtures.
- [ ] Client participates in `./dev test client-real-e2e-parity` using the shared corpus.
- [ ] `./dev test <client>` passes.
- [ ] `./dev check-fast` and `./dev check` pass.
- [ ] `./dev universe` passes for final first-class qualification, locally or through supported exact-HEAD remote execution where applicable.
- [ ] Independent adversarial review completed.

A green mandatory PR gate alone is not sufficient evidence for this checklist.

---

# Part XIII — Final Invariants

## 70. The client architecture must remain boring

A successful new client should ultimately converge on the canonical dispatcher
through one deliberately selected representation path.

Default binary/native path:

```text
application code
      ↓
small idiomatic facade
      ↓
standard generated FlatBuffers objects
      ↓
canonical FlatBuffer envelope bytes
      ↓
generated gRPC transport
      ↓
searchkerneld
      ↓
shared dispatcher
      ↓
searchkernel-ir
      ↓
Vespa
```

Explicit HTTP-first path:

```text
application code
      ↓
small idiomatic facade
      ↓
standard FlatBuffers JSON envelope
      ↓
generic HTTP execute edge
      ↓
official BFBS-backed JSON parser
      ↓
canonical FlatBuffer envelope bytes
      ↓
the same shared dispatcher
      ↓
searchkernel-ir
      ↓
Vespa
```

For clients that claim managed-local operation, add only this above it:

```text
Index.create/open
      ↓
ManagedLocalHost
      ↓
spawn/own searchkerneld
      ↓
normal client stack
```

The language changes.

The portable contract, semantics, and engine do not.

That is the standard to preserve for every future Search Kernel client.
