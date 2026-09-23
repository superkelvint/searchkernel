# Search Kernel PHP client

This is the first HTTP/JSON Search Kernel client slice.

The public PHP facade is ergonomic, but the HTTP body is **standard FlatBuffers
JSON for the canonical Search Kernel envelope**. The daemon converts it with the
existing BFBS-backed FlatBuffers JSON bridge before calling the same dispatcher
used by gRPC.

No PHP-specific Search Kernel domain schema exists.

## Architectural direction

PHP is intentionally an HTTP-first Search Kernel client. The long-term path is
not to grow a PHP REST API; it is to make the PHP facade broad and idiomatic
while keeping the wire representation mechanically tied to the canonical
FlatBuffers contract:

```text
PHP ergonomic API
    ↓
schema-derived standard FlatBuffers JSON
    ↓
POST /v1/execute
    ↓
BFBS-backed FlatBuffers JSON parser
    ↓
canonical RequestEnvelope bytes
    ↓
shared Dispatcher
    ↓
searchkernel-ir / *_to_ir()
    ↓
Vespa
```

Responses take the inverse path through the canonical FlatBuffers response
encoder and BFBS-backed JSON printer.

The PHP client may own host-language ergonomics: builders, value conversion,
result wrappers, error presentation, endpoint configuration, packaging, and
tests. It must not own Search Kernel defaults, filter parsing/semantics, schema
validation, ranking/grouping semantics, request normalization, or backend
behavior.

The intended expansion order is:

1. complete the portable operation core and centralize request/response union
   mapping;
2. make portable value conversion lossless across int64/uint64, bytes, lists,
   objects, tensors, duplicate-key rejection, and future union variants;
3. port the shared result tree, filters, query inputs, ranking, grouping/facets,
   patch/update, analyze, describe, and inspect-query ergonomics;
4. add package-level low-level access to schema-derived JSON/envelope helpers so
   advanced callers are not blocked on ergonomic sugar;
5. add real-engine PHP E2E and cross-language conformance coverage;
6. harden Composer packaging, endpoint/deadline behavior, and compatibility
   policy.

Managed local daemon lifecycle is a separate concern from the portable PHP API.
It should only be added if PHP deployments materially benefit from PHP spawning
and supervising `searchkerneld`; remote/shared-host deployments can connect to
an already managed daemon without changing the client semantics.

This HTTP direction does not preclude a future generated gRPC PHP transport.
Both transports can coexist because neither owns domain semantics; they terminate
at the same canonical FlatBuffer dispatcher.

## Current first slice

Supported ergonomically:

- `Index::connect()`
- `insert()`
- `upsert()`
- `get()`
- `delete()`
- `search()->limit()->execute()`
- scalar/list/object portable values used by those operations

The full first-party facade, generated low-level escape hatch, managed local
lifecycle, tensors/bytes, filters, grouping, ranking, and the rest of the client
SOP are intentionally deferred from this first proof.

Run unit tests:

```bash
cd clients/php
php tests/run.php
```

Run the real-engine showcase against a `searchkerneld` instance whose
`--web-listen` address is reachable and whose schema has a searchable/stored
`title` text field:

```bash
SEARCHKERNEL_HTTP_URL=http://127.0.0.1:50052 \
  php examples/try_searchkernel_client_api_e2e.php
```
