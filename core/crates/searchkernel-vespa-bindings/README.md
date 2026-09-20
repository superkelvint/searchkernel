# `searchkernel-vespa-bindings`

This crate provides raw Rust bindings to the Search Kernel private native ABI.
It links the generated/prebuilt native SDK and exposes opaque handles,
constants, and function declarations to `searchkernel-vespa-engine`.

## Start reading here

1. Read `src/abi.rs` for the expected ABI version.
2. Read `src/bindings.rs` for the generated C declarations and symbols.
3. Read `build.rs` to understand SDK discovery and ABI checks.
4. Read `core/native/vespa/searchkernel_vespa.h` for the native declaration.

## Key concepts

- This crate is intentionally raw and unsafe. Callers must uphold the C ABI.
- It must not expose Vespa C++ types to the product-facing Rust API.
- The public ABI intentionally exposes opaque handles and fixed-width
  primitives rather than concrete aggregate layouts. Text data uses
  structure-of-arrays arguments, so a C/Rust field-layout contract and its
  borrowed-pointer lifetime contract cannot drift independently.
- Numeric constants and function signatures are compatibility contracts; the
  release-level shared-library ABI is checked with libabigail as documented in
  `../../native/ABI.md`.
- SDK contents are generated/prebuilt artifacts; do not hand-edit them.
- `src/bindings.rs` is checked in and used by normal builds. Run
  `tools/check-native-bindings.sh` to compare it with an explicit bindgen
  regeneration; this keeps libclang out of the normal build graph.
- The C++ contract helpers are compiled only by
  `tools/test-native-bindings.sh` through the `native-contract-tests` feature.
