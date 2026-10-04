# Vespa native adapter

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
