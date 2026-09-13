# Vespa native adapter

This directory is the Search Kernel-owned C/C++ adaptation layer around the
pinned Vespa engine. It is where Vespa-specific headers, object lifetimes,
configuration details, and native error translation belong.

## Start reading here

1. Read `searchkernel_vespa.h` to understand the narrow ABI.
2. Read `searchkernel_vespa.cpp` in the same order as the header functions.
3. Read `embedded_crud_cfg/` to see the configuration consumed by the
   embedded engine.
4. Read `build-sdk.sh` only when a native SDK rebuild is required.
5. Read `../ABI.md` for the required debug-info and ABI comparison workflow.

## Key concepts

- The adapter is the only place that should know Vespa C++ types.
- Every opaque handle has a clear create/use/destroy path.
- ABI status codes and error payloads are the only native behavior visible to
  Rust.
- The pinned upstream checkout is read-only; adapt Vespa here instead.
