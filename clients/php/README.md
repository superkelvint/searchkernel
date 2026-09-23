# Search Kernel PHP client

This is the first HTTP/JSON Search Kernel client slice.

The public PHP facade is ergonomic, but the HTTP body is **standard FlatBuffers
JSON for the canonical Search Kernel envelope**. The daemon converts it with the
existing BFBS-backed FlatBuffers JSON bridge before calling the same dispatcher
used by gRPC.

No PHP-specific Search Kernel domain schema exists.

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
