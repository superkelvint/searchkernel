# M11 — Low-Level Wrapper Hardening

M11 hardens the standalone `searchkernel-vespa-engine` wrapper and the current M0–M7
native surface. It adds no search semantics and has no dependency on the
C-series. M8–M10 are not present in this checkout and are intentionally not
recreated here.

The safety contract is:

- `Engine` is `Send + Sync`; supported operations may overlap on one engine.
- Lifecycle transitions are exclusive. Close prevents new calls and waits for
  active calls before native resources are released.
- Searches may overlap with searches and mutations. Mutations may overlap with
  one another; the adapter serializes only first-use bucket preparation and
  allocates mutation timestamps atomically, while Vespa owns feed-operation
  concurrency.
- Native handles have one owner and one destruction path. `Drop` never panics.
- All ABI entry points are `noexcept`; C++ exceptions become status errors.
- Borrowed Rust buffers are used only for the duration of their call.
- Native result memory remains valid until its matching result destroy call.
- Invalid values within ABI memory preconditions produce controlled errors.

The fixed structural limits are 256 query nesting levels, 65,536 query nodes,
and 4 MiB of YQL input. Existing text, identifier, term, occurrence, and
search limits remain authoritative.

Acceptance uses the real pinned Vespa engine, the safe Rust wrapper, and the
instrumented native adapter. It does not use a mock search backend or a second
semantic search oracle.
