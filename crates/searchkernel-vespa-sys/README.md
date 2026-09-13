# `searchkernel-vespa-sys`

This is the raw Rust binding crate for the Search Kernel private native ABI.
It links the generated/prebuilt native SDK and exposes opaque handles,
constants, structs, and function declarations to the safe Vespa adapter.

## Start reading here

1. Read `src/abi.rs` for the expected ABI version.
2. Read `src/bindings.rs` for the generated C layout and symbols.
3. Read `build.rs` to understand SDK discovery, bindgen, and layout checks.
4. Read `native/vespa/searchkernel_vespa.h` for the native declaration.

## Key concepts

- This crate is intentionally raw and unsafe. Callers must uphold the C ABI.
- It must not expose Vespa C++ types to the product-facing Rust API.
- Struct layout and numeric constants are compatibility contracts.
- SDK contents are generated/prebuilt artifacts; do not hand-edit them.
