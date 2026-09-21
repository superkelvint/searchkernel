# Native build provenance

For day-to-day commands and prerequisite handling, start with
[`BUILDING.md`](../BUILDING.md). This document records the provenance model
that makes those commands safe against warm native caches.

Search Kernel has two native build layers that must not share artifacts across
different environments:

```text
Cargo target directory
  ├── FlatBuffers C++ build output
  └── native-sdk/linux-x86_64/   Vespa adapter SDK
```

The supported environment roots are:

| Environment | Cargo target root | Vespa SDK |
| --- | --- | --- |
| Host | `target/host` | `target/host/native-sdk/linux-x86_64` |
| Project container | `target/devcontainer` | `target/devcontainer/native-sdk/linux-x86_64` |
| CI | `target/ci` | `target/ci/native-sdk/linux-x86_64` |

`.cargo/config.toml`, the project container, and CI select these roots. An
explicit `CARGO_TARGET_DIR` selects a separate root; the Rust native bindings
derive the corresponding SDK path from Cargo's `OUT_DIR`. `SK_NATIVE_SDK` is
available for deliberately custom SDKs and should point to an isolated absolute
or repository-root-relative path.

## What is recorded

Both native layers record a deterministic provenance fingerprint containing the
inputs that can affect binary compatibility, including:

- Rust host/toolchain and Cargo target;
- canonical C/C++ compiler paths, versions, and target/dumpmachine output;
- archiver, ranlib, linker, libc, and C++ runtime identities;
- relevant compiler, linker, CMake, standard-library, sanitizer, coverage, and
  native configuration flags;
- the pinned FlatBuffers source identity for the FlatBuffers build;
- the pinned Vespa revision and configured Vespa build inputs for the SDK; and
- the Search Kernel native source/header/configuration content.

The record contains no timestamps, process IDs, temporary paths, or random
values. The native SDK also stores its fingerprint digest in `manifest.json`.

## Build behavior

For FlatBuffers, a changed fingerprint invalidates only the FlatBuffers CMake
output under that Cargo package's `OUT_DIR`. A parser compile/link probe then
checks the actual `libflatbuffers.a` archive. A failed probe causes one clean
rebuild of that FlatBuffers sub-build and one retry.

For the Vespa SDK, `core/native/vespa/build-sdk.sh` publishes atomically through
a staging directory and records provenance with
`tools/native-build-fingerprint.py`. The Rust bindings verify the SDK record
and compile/link a probe that calls `sk_native_abi_version()` from the actual
SDK library. A missing, stale, or incompatible SDK causes one automatic,
targeted invocation of `build-sdk.sh`; a second failure stops before the Rust
link step and reports the diagnostics.

Neither path runs `cargo clean`, deletes the entire target directory, or
reuses an SDK from another supported environment. The Vespa SDK rebuild still
requires the pinned Vespa checkout's configured build outputs and dependency
installation; when those prerequisites are absent, the build fails with that
specific prerequisite error rather than a downstream linker error.

Useful verification commands are:

```bash
bash tools/test-native-environment-fingerprint.sh
bash tools/test-native-build-hardening.sh
cargo test --workspace --all-targets --no-default-features
```

The first verifier exercises stable fingerprints, compiler-wrapper changes,
flag changes, and host/container/CI target separation. The second verifies
FlatBuffers warm reuse, targeted invalidation, preservation of unrelated Rust
artifacts, and one-shot recovery from a broken archive.
