# M11 v2 — Full-Surface Wrapper Hardening

M11 hardens the standalone `searchkernel-vespa-engine` wrapper across the
complete M0–M10 native surface: sorting, document summaries, results, grouping,
facets, attribute mutation, and ranking. The same torture workload also runs
through the real `searchkernel-ir-adapter` and direct native engine lifecycle and
call entry points. M11 adds no search semantics.

The safety contract is:

- `Engine` is `Send + Sync`; supported operations may overlap on one engine.
- Lifecycle transitions are exclusive. Close prevents new calls and waits for
  active calls before native resources are released.
- Searches may overlap with searches and mutations. Mutations may overlap with
  one another; the adapter serializes only first-use bucket preparation and
  allocates mutation timestamps atomically, while Vespa owns feed-operation
  concurrency.
- Successful supported mutations are publication fences: the immediate GET and
  search on the same engine observe the completed mutation. Source-document or
  retriever visibility alone is insufficient; the native boundary initiates a
  feed-view force-commit on Vespa's master executor and waits for its completion
  callback on the caller thread, preserving master-thread feed concurrency.
- Native handles have one owner and one destruction path. `Drop` never panics.
- All ABI entry points are `noexcept`; C++ exceptions become status errors.
- Borrowed Rust buffers are used only for the duration of their call.
- Native result memory remains valid until its matching result destroy call.
- Invalid values within ABI memory preconditions produce controlled errors.

The fixed structural limits are 256 query nesting levels, 65,536 query nodes,
and 4 MiB of YQL input. Existing text, identifier, term, occurrence, and
search limits remain authoritative.

Acceptance uses the real pinned Vespa engine, the safe Rust wrapper, and the
instrumented native adapter, the real IR adapter, and production FFI symbols.
The driver contains only a deterministic state model and wire builders; it does
not use a mock search backend or a second semantic search oracle. Failpoints are
test-SDK-only and are never part of the production SDK.
