# Document workflow

The daemon adapter owns the generation-bound SDK `Index`. Documents uses the
same prepared search requests and URL codec as Search Lab: an empty query is
canonical match-all, with explicit backend limit/offset. Advanced query state
can be carried between Documents and Search Lab without another query language.

Detail reads use the public client's decoded `GetResult`, separately from search
hit projections. Its complete canonical JSON is inspectable. GET-derived editor
fields use canonical `Value` objects, preserving integer width, containers and
tensor types/payloads; display conversion never becomes a mutation payload.
Ordinary JSON input uses the shared SDK `jsonInput` conversion, with explicit
canonical values available for types JSON alone cannot express.

`src/api/document-request.ts` constructs canonical logical requests through the
SDK's schema-driven decoder. The prepared request is retained privately; the
inspector describes the exact object executed by the generation-bound public
client. Portable schema validation and operation meaning remain in IR.

Supported writes are create-only insert, atomic full-document upsert, patch
set/unset, update assignment/clear/increment/decrement/multiply/divide (including
update-create), and confirmed delete. Complete dense tensor assignment is
supported. Other canonical update unions remain available through the advanced
operations editor and return the backend's errors where unsupported:

- `core/crates/searchkernel-ir/src/api_to_ir/document.rs`: collection add/remove/
  match and addressed tensor modify return `update_operation_not_supported`.
- `core/crates/searchkernel-ir/src/api_to_ir/resolve/document.rs`: only simple
  field update paths execute; nested paths are unsupported.
- `core/crates/searchkernel-ir-adapter/src/backend.rs`: conditional upsert/
  update/delete return `conditional_mutation_not_supported`.
- `core/crates/searchkernel-ir-adapter/src/lib.rs`: native document values
  support dense tensors; sparse/mixed payloads are unsupported.
- `core/crates/searchkernel-ir-adapter/src/schema.rs`: native default summaries
  omit tensors and arrays of structs. GET retains complete stored values; the
  browser does not manufacture those fields in search-hit projections.

The shared IR projection resolver owns the native search-summary capability
classification used by schema lowering. Position, Predicate, Tensor and stored
arrays of structs stay out of omitted search projections; explicit requests
return the corresponding canonical unsupported error with its projection path.
GET's omitted/explicit projections retain these stored fields. The native
adapter consumes the same IR classification instead of maintaining a second
summary-capability list.

`mutateAndVerify` dispatches the write without binding it to component
cancellation, then performs cancellable fresh GET and search reads outside React
Query caches. It verifies only the presence/absence postcondition that can be
checked without reimplementing update semantics in the browser, and exposes the
actual read responses. The real-engine E2E independently checks concrete field
effects. If read-back fails after acknowledgement, the error records that fact
and forbids retry. If the mutation response itself is lost through a
transport/protocol failure, the outcome is marked unknown and retry is blocked
until an authoritative GET reconciles the current document state. A delete
response with `deleted=false` is reported as a no-op rather than a successful
deletion.

Document IDs use an explicit `by-id/u.<base64url UTF-8>` route, avoiding router
percent/slash decoding ambiguity. Existing document routes continue to work.
Unsaved editor contents never enter the URL. Reloading creates a fresh adapter
and opens the target canonically; stale handles are never silently retried.

Run the registered container pathways through exact-SHA CI verification:

```sh
./dev test frontend
./dev test frontend-real-engine
./dev test typescript
```

The W2 Chromium test creates a real index, performs each supported write through
the application, checks fresh GET/search/filter results, restores opaque detail
URLs in new pages, and restarts the daemon three times to prove persistence and
durable deletion. Unit coverage checks exact canonical requests, typed values,
ID encoding, error inspectors, and acknowledgement/read-back ordering/failures.
The immutable F-series acceptance artifacts are independent and unchanged.
