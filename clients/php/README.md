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

- `Index::connect()`, insert/upsert/get/delete, patch and advanced update;
- search, browse, typed query, and raw YQL entry points;
- textual filters with clause-local typed bindings;
- field/relevance/feature sorting, projection, highlighting, offset/limit/page;
- ranking profiles, query inputs, rank properties, and generic properties;
- facets, root aggregations, structured/nested grouping, aliases, ordering,
  grouping keep filters, and summary aggregation;
- recursive hit/group result decoding, facets/aggregations projections,
  match/rank features, coverage/degradation, timing, result errors, and
  continuations;
- portable null/bool/int64/uint64/float64/string/bytes/list/object/tensor values,
  including explicit `UInt64` and `ValueObject` wrappers for PHP-ambiguous
  values;
- strict transport, protocol, and portable-error domains with malformed wire
  data rejected rather than coerced.

The typed query/grouping helper vocabulary is intentionally the subset required
by the current canonical E2E. New ergonomic helpers must continue to lower
directly to the canonical FlatBuffers JSON shapes rather than implementing
query/grouping semantics in PHP.

## Lifecycle boundary

PHP does not currently expose Python-style `Index::create()` /
`Index::open()` schema construction and managed-daemon ownership. Index
lifecycle/schema administration is outside the cross-language client API spec,
and this HTTP client normally connects to an already managed `searchkerneld`.

For the canonical real-engine E2E, the harness creates the exact same
schema/rank-profile fixture through the existing canonical Python managed-local
path, then starts `searchkerneld` with its HTTP edge and exercises only the
public PHP facade. The harness restarts the daemon against the same index and
runs PHP reopen assertions, so persistence is verified without inventing PHP
schema semantics.

Grouping continuation execution remains deferred for the same backend reason as
the Python showcase: the native grouping ABI does not currently return
continuation tokens. Decoding preserves tokens when they are present.

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
