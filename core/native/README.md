# Native components

The native side packages the Vespa engine behind a Search Kernel-owned C ABI.

## Start reading here

1. Read `vespa/searchkernel_vespa.h` for the ABI declarations.
2. Read `vespa/searchkernel_vespa.cpp` for the adapter implementation.
3. Read `vespa/manifest.json` and `vespa/build-sdk.sh` for packaging.
4. Read the Rust sys crate to see how the ABI is consumed.
5. Read `ABI.md` for the release-baseline and libabigail comparison workflow.

## Key concepts

- Vespa headers and implementation types stay inside the native adapter.
- The ABI uses opaque handles and Search Kernel-owned values.
- Native exceptions are caught and translated into error values.
- `core/native/sdk` is generated/prebuilt output used by ordinary Rust builds.

Native changes require the repository's SDK build and conformance workflow;
ordinary Rust-only changes should not rebuild Vespa.
