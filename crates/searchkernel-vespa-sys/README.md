# `searchkernel-vespa-sys`

This is the raw Rust binding crate for the Search Kernel private native ABI.
It links the generated/prebuilt native SDK and exposes opaque handles,
constants, and function declarations to the safe Vespa adapter.

## Start reading here

1. Read `src/abi.rs` for the expected ABI version.
2. Read `src/bindings.rs` for the generated C declarations and symbols.
3. Read `build.rs` to understand SDK discovery and bindgen drift checks.
4. Read `native/vespa/searchkernel_vespa.h` for the native declaration.

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
