# Native components

The native side packages the Vespa engine behind a Search Kernel-owned C ABI.

## Start reading here

1. Read `vespa/searchkernel_vespa.h` for the ABI declarations.
2. Read the focused files under `vespa/` for the adapter implementation;
   `abi.cc` contains the exported entry points and the subsystem files contain
   their private Vespa integration.
3. Read `vespa/manifest.json` and `vespa/build-sdk.sh` for packaging.
4. Read `core/crates/searchkernel-vespa-bindings/README.md` to see how Rust
   consumes the ABI.
5. Read [`../../docs/native-build-provenance.md`](../../docs/native-build-provenance.md)
   for target isolation, SDK provenance, and automatic recovery.
6. Read `ABI.md` for the release-baseline and libabigail comparison workflow.

## Key concepts

- Vespa headers and implementation types stay inside the native adapter.
- The ABI uses opaque handles and Search Kernel-owned values.
- Native exceptions are caught and translated into error values.
- Native SDK output is generated under the active environment's Cargo target
  directory (`target/host/native-sdk`, `target/devcontainer/native-sdk`, or
  `target/ci/native-sdk`) and is never shared across those environments.
- Each SDK contains a native-build fingerprint. Rust bindings verify that the
  fingerprint and a real ABI link probe match the current compiler/runtime
  before linking it.

Native changes require the repository's SDK build and conformance workflow;
ordinary Rust-only changes should not rebuild Vespa.

<!-- CI smoke: current merged native verifier closure; do not merge. -->
