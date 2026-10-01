# Search Kernel PHP client

The PHP client is an HTTP-first first-party facade over Search Kernel's
canonical portable contract.

The HTTP body is **standard FlatBuffers JSON for the canonical Search Kernel
envelope**. `searchkerneld` converts it with the BFBS-backed FlatBuffers JSON
bridge before calling the same dispatcher and `*_to_ir()` semantics used by
other transports.

No PHP-specific Search Kernel domain schema exists.

## Architecture

```text
PHP ergonomic API
    ↓
standard FlatBuffers JSON RequestEnvelope
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

Responses take the inverse path through the canonical response encoder and
FlatBuffers JSON printer.

PHP owns only host-language ergonomics, lossless value conversion, result
wrappers, error presentation, endpoint configuration, packaging, and tests. It
must not own Search Kernel defaults, filter parsing/semantics, schema validation,
ranking/grouping semantics, request normalization, or backend behavior.

## Current ergonomic surface

The current branch covers the public operations exercised by the canonical
client E2E:

- `Index::connect()`, insert/upsert/get/delete, patch and advanced update,
  including matched-element and tensor-modify request unions;
- search, browse, typed query, raw YQL, analyze, describe, and query inspection;
- textual filters with clause-local typed bindings and explicit search-field
  selection/weights;
- field/relevance/feature/raw sorting, projection, highlighting,
  offset/limit/page, timeout, and presentation controls;
- ranking profiles, literal/expression query inputs, rank properties, generic
  properties, query-model controls, query profile, and search chain;
- facets, root aggregations, raw grouping, structured/nested grouping,
  functions, aliases, arithmetic, ordering, finite/infinite limits, precision,
  grouping filter/keep/where, continuation request tokens, and summary/XOR/
  quantile aggregations;
- recursive hit/group result decoding, facets/aggregations projections,
  match/rank features, coverage/degradation, timing, result errors, and
  continuations;
- portable null/bool/int64/uint64/float64/string/bytes/list/object/tensor values,
  including explicit `UInt64` and `ValueObject` wrappers for PHP-ambiguous
  values;
- strict transport, protocol, and portable-error domains with malformed wire
  data rejected rather than coerced.

Typed-query helpers cover the canonical boolean, comparison/range/IN,
phrase/proximity, weighted retrieval, rank, labels/non-empty, geo,
nearest-neighbor, predicate, user-input, text, and annotation families. These
helpers and the grouping facade only construct canonical portable unions;
validation, defaults, normalization, and backend capability decisions remain
server-owned.

`YqlBuilder` is intentionally narrower than `SearchBuilder`: it exposes
request-level controls that may coexist with raw YQL (properties, grouping,
ranking, inputs, pagination, timeout, execute/inspect) and does not expose
competing high-level filter, sort, search-field, or projection methods.

## Lifecycle boundary

PHP supports create/open/delete/select for the canonical multi-index daemon
lifecycle on an externally managed `searchkerneld`. `Index::create()` creates
a logical target from an `IndexDefinition`, `Index::open()` reopens an existing
target, and `Index::deleteTarget()` removes it. Ordinary operations carry the
same target in the canonical request envelope. PHP schema builders only
construct the `IndexDefinition`; validation, defaults, and backend
semantics remain server-owned. Local handle-close ownership semantics are not
introduced by this slice.

The real-engine E2E starts one `searchkerneld --data-root`, creates the schema
and rank profiles through PHP itself, restarts the daemon, and reopens the same
logical target through PHP. It has no Python client/schema bootstrap dependency.
Transparent PHP-owned daemon spawning remains out of scope: callers may still
use `Index::connect()` with an already managed daemon.

Grouping continuation execution remains deferred for the same backend reason as
the Python showcase: the native grouping ABI does not currently return
continuation tokens. Decoding preserves tokens when they are present.

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

## Verification

Focused PHP semantic/wire tests:

```bash
php clients/php/tests/run.php
```

Canonical real-engine E2E, including daemon restart/reopen:

```bash
bash clients/php/examples/run_searchkernel_client_api_e2e.sh
```

The E2E expects a native `searchkerneld` at `SEARCHKERNEL_LOCAL_HOST` or the
repository's normal debug target. CI registers this script as the PHP executable
capability lane, so E2E evidence cannot be green without running it.

To regenerate/check the cross-language capability report:

```bash
python3 tools/check-client-contracts.py --write
python3 tools/check-client-contracts.py --check
```

## Packaging

The Composer package uses PSR-4 autoloading under `SearchKernel\` and supports
PHP 8.1+. Package/install consumption remains a separate first-class-client gate;
do not infer publish readiness solely from the source-tree tests.
