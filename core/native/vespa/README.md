# Vespa native adapter

W5 diagnostic searches attach `final_matcher_request` to the existing raw trace
immediately before `DocumentDB::match()`. `searchkernel.proton-request.v1` is
private engine evidence, never a portable request or a client semantic source.
The capture reads the populated request and includes manifest-derived Vespa
revision/native ABI provenance, matcher/sort/grouping/trace/profiler controls,
and bounded serialized-tree item types/arity/IDs/flags with terms omitted.
Property keys are selected lexically (at most 32 per collection). Values,
including scalar query features and binary tensors, are redacted except for the
adapter-owned single matching thread control. Keys over 128 bytes and sorting
over 256 bytes are omitted. The existing raw-byte budget caps final encoding
and bounds tree/property collection. Structural or encoding truncation sets
diagnostic truncation. Ordinary searches skip capture. `./dev test native-crud`
tests post-lowering values, privacy, bounds, deterministic property selection
and unchanged ordinary-search results against real Vespa.

This directory is the Search Kernel-owned C/C++ adaptation layer around the
pinned Vespa engine. It is where Vespa-specific headers, object lifetimes,
configuration details, and native error translation belong.

## Start reading here

1. Read `searchkernel_vespa.h` to understand the narrow ABI.
2. Read the focused adapter modules: `engine.cc`, `document.cc`, `update.cc`,
   `query.cc`, `search.cc`, `grouping.cc`, `tensor.cc`, `errors.cc`,
   `hardening.cc`, and `abi.cc` for the exported entry points.
3. Read `embedded_crud_cfg/` to see the configuration consumed by the
   embedded engine.
4. Read [`../../../docs/native-build-provenance.md`](../../../docs/native-build-provenance.md)
   for the environment-specific SDK paths and automatic rebuild behavior.
5. Read `build-sdk.sh` only when a native SDK rebuild is required.
6. Read `../ABI.md` for the required debug-info and ABI comparison workflow.
7. Read [per-hit match evidence](../../../docs/vespa-match-evidence.md) before
   adding query diagnostics that depend on Proton match-data lifetimes.

## Key concepts

- The adapter is the only place that should know Vespa C++ types.
- Every opaque handle has a clear create/use/destroy path.
- ABI status codes and error payloads are the only native behavior visible to
  Rust.
- The pinned upstream checkout is read-only; adapt Vespa here instead.
- `build-sdk.sh` writes only to the selected SDK/build paths and records the
  native environment that produced them. The Rust bindings reject a stale or
  incompatible SDK before the Rust linker sees it.

## Native adapter coverage

Run `tools/coverage-native-adapter.sh` from the repository root. It builds an
O0, gcov-instrumented adapter with hardening failpoints, runs the direct C++
ABI harness plus the native Rust integration and M11 scenarios, and requires
at least 90% executable line coverage across the split adapter sources. The
configured Vespa dependency installation must be available at the path
recorded by the pinned Vespa build, or supplied with `VESPA_DEPS`,
`VESPA_DEPS_INCLUDE_DIR`, and `VESPA_DEPS_LIB_DIR`.

## ABI 17 structured diagnostic projection

`diagnostic_projection.h` is deliberately pinned to the revision in
`reference/vespa/PINNED_VESPA.json`; native build tooling verifies that the
manifest-derived `SK_PINNED_VESPA_COMMIT` agrees with it. The result owns five
independent projection-v1 sections. `sk_generic_search_result_diagnostic_section`
borrows their UTF-8 Search Kernel schema buffers until result destruction;
immutable concurrent reads are permitted. Rust copies/decodes before freeing.
The schema contains provenance and flat preorder nodes with parent and thread
indices, name, optional count, total milliseconds, and self milliseconds.
This internal schema is not arbitrary Slime or a public JSON domain contract.

Exact engine authorities at this pin:

- `Matcher::traceQuery(6, ...)` writes `traces[tag=query_execution_plan].optimized`
  using `Blueprint::asSlime()`.
- `MatchMaster::match()` / `LazyTraceInserter::handle_thread()` writes
  `traces[tag=query_execution].threads[*].traces`.
- Within each thread, `MatchThread::run()` emits `ExecutionProfiler::report()`
  into the `match_profiling`, `first_phase_profiling`, and `second_phase_profiling`
  tagged entries. The tree report has `profiler=tree`, `depth`, `total_time_ms`,
  and optional `roots`. Nodes have name/count/total_time_ms, and self_time_ms
  plus children when children exist. Leaf self time is absent, not inferred.
- `MatchThread::findMatches()` at trace level 6 writes `iterator.optimized` using
  `SearchIterator::asSlime()`, before inserting profiling wrappers.

Profiler owners are private matcher internals; blueprint and iterator owners
also do not escape the match. Their native Slime reports are the available
post-execution authority, so all five sections depend on these pinned trace
paths. Extraction only visits those paths, never arbitrary matching keys.
Reports from separate threads remain separate roots. The optional portable
`thread_total_seconds_sum` adds only captured report-root totals, not descendants;
it is aggregate profiled thread time and cannot be interpreted as wall latency.

Topology uses Object2Slime's `[type]` names, numbered `children` vector members,
unary `search` edges, and SourceBlender's explicit `Source N` edges. Non-topology
members (field lists, hit estimates, costs, term internals) are not projected.
Specialized links outside these visitor shapes are not represented: these trees
are bounded structural evidence, not a claim of complete query causality or
per-hit execution. Negative-depth flat/top-N profiling is not enabled by this
ABI; an unexpected report kind fails closed. Upstream is unchanged.

Each section supports Available, Unsupported, UnavailableForQuery, Truncated,
TimedOut, and Failed. These five sources are supported by this pin; an absent
report or an empty profiler report is UnavailableForQuery, not Unsupported.
Missing evidence after an engine timeout is TimedOut. Malformed source structures
are Failed and contain no trusted partial nodes. Available evidence survives
raw byte/depth truncation. Unrequested profiler controls yield no profile nodes.

Bounds are independent: <=128 nodes, <=256 bytes per name, <=65536 encoded bytes
per section, <=1024 inspected trace entries or tree nodes, profiler depth <=16,
structural depth <=32. A pruned section is Truncated. Raw trace still obeys its
request byte budget (<=262144), structural depth, and a 4096-value work ceiling.
Checks precede recursive raw encoding. All construction/allocation sizes are
bounded even when a name needs JSON escaping; an encoded section overflow retains
provenance with an empty node list. Raw payload media type reflects truncation.

The ABI revision is 17 because the diagnostic control setter adds a structural
depth parameter. ABI authority metadata and generated bindings/consumer bundle
must agree. Historical frozen ABI fixtures change only their revision numbers.
An external release baseline is required for binary ABI comparison; it is never
regenerated to bless this change.
