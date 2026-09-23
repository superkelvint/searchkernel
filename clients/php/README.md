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

## Target managed deployment architecture

The long-term PHP deployment target should make local Search Kernel feel
self-contained without moving the Vespa engine into PHP-FPM or Apache worker
processes.

The preferred shape is:

```text
PHP / WordPress / Drupal
        ↓
Search Kernel PHP client
        ↓
managed local transport
        ↓
one bundled searchkerneld process
        ↓
Search Kernel / Vespa native libraries
        ↓
index directory
```

The PHP package may bundle a compatible `searchkerneld` binary and its private
Search Kernel/Vespa shared-library closure. The PHP client, an optional PHP
extension, or framework integration may ensure that daemon is running and then
connect to it over the supported local daemon transport. The transport used by
PHP may evolve; managed deployment must not create a second Search Kernel domain
or semantic path.

### Do not embed the engine in PHP workers

A future `searchkernel.so` PHP extension must not load and own a separate
Vespa/Search Kernel engine inside each PHP-FPM or Apache worker merely to avoid a
daemon process.

PHP-FPM and Apache commonly use multiple worker processes and recycle them over
time. In-process engine ownership would therefore risk multiple engine instances
opening the same index, duplicated memory and background threads, unclear write
ownership, unsafe fork/lifecycle behavior, and engine lifetime becoming coupled
to web-worker recycling.

If a PHP extension is provided, its preferred role is therefore packaging and
managed-daemon ergonomics:

```text
PHP
 ↓
searchkernel.so
 ↓
ensure compatible bundled searchkerneld
 ↓
connect to managed daemon
```

The daemon remains the single engine-owning process.

### Managed daemon lifecycle

Managed startup must be safe when many PHP workers discover an absent daemon at
the same time. The intended lifecycle is conceptually:

```text
Index::open(...)
    ↓
check private endpoint
    ↓
healthy compatible daemon? ── yes ──> connect
    │
    no
    ↓
acquire per-index startup lock
    ↓
check again
    ↓
spawn bundled searchkerneld
    ↓
wait for readiness
    ↓
connect
```

The eventual implementation should define and verify:

- one engine-owning daemon per managed index/runtime identity;
- race-safe startup locking and a second readiness check after acquiring it;
- stale PID/socket detection and recovery;
- daemon/protocol/package compatibility checks before reuse;
- deterministic index ownership and refusal of incompatible concurrent owners;
- graceful shutdown semantics and, if useful, bounded idle shutdown;
- behavior across PHP-FPM/Apache worker restart and recycling;
- persistence/reopen behavior when the managed daemon exits and restarts;
- private endpoint permissions suitable for shared-host installations;
- diagnostics that distinguish daemon-startup failures from portable Search
  Kernel operation errors.

Process supervision semantics should live in reusable Search Kernel managed-daemon
machinery where possible rather than being reinvented independently by PHP,
WordPress, Drupal, Python, or another client.

### Packaging targets

The same daemon architecture can support several deployment tiers.

1. **Ordinary PHP / WordPress / Drupal package.** Bundle the PHP SDK plus a
   platform-specific `searchkerneld` and private native-library closure. This is
   the lowest-friction option where the host permits executing bundled native
   binaries.
2. **Optional PHP extension package.** Ship `searchkernel.so` together with the
   matching daemon and native libraries. The extension improves installation or
   lifecycle ergonomics but still delegates engine ownership to
   `searchkerneld`.
3. **Explicit system-managed daemon.** Operators may install and supervise
   `searchkerneld` themselves and configure PHP to connect to it. This remains
   useful for hosting providers and deployments that want explicit service
   ownership.

These are deployment choices, not separate client semantics.

Shared-host environments may prohibit process spawning, arbitrary native
binaries, or custom PHP extensions. Search Kernel should therefore retain the
ability to connect to an already managed daemon rather than making transparent
local spawning mandatory.

### WordPress and Drupal

Future WordPress and Drupal integrations should consume the ordinary PHP client
and this managed-daemon deployment facility rather than introducing their own
engine boundary:

```text
WordPress plugin ─┐
                  ├─> PHP client ─> managed searchkerneld ─> Vespa
Drupal module ────┘
```

CMS-specific code owns content/entity mapping, indexing hooks, queues,
configuration, administration, and reindex workflows. It does not own portable
Search Kernel semantics or a CMS-specific native engine path.

This section records a target deployment architecture only. It does not claim
that bundled daemon lifecycle, a PHP extension, or CMS packaging is implemented
today.

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
