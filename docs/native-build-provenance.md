# Native build provenance

For day-to-day commands and prerequisite handling, start with
[`BUILDING.md`](../BUILDING.md). This document records the provenance model
that makes those commands safe against warm native caches.

Search Kernel has two native build layers with different cache scopes:

```text
Cargo target directory
  └── FlatBuffers C++ build output       worktree/environment-local

Git common directory
  └── searchkernel/native-sdk-cache/
      └── <environment>/<target>/objects/<fingerprint>/
                                            immutable Vespa adapter SDK
```

Cargo/FlatBuffers outputs are never shared across environments. Vespa SDK
objects may be shared by worktrees only when their complete native provenance
fingerprint is identical.

The supported environment roots are:

| Environment | Cargo target root | Shared SDK namespace |
| --- | --- | --- |
| Host | `target/host` | `native-sdk-cache/host/<target>/...` |
| Project container | `target/devcontainer` | `native-sdk-cache/devcontainer/<target>/...` |
| CI | `target/ci` | `native-sdk-cache/ci/<target>/...` |

`.cargo/config.toml`, the project container, and CI select the Cargo roots.
The Vespa SDK resolver uses `git rev-parse --git-common-dir` so all worktrees
from one clone see the same object store, while `SEARCHKERNEL_BUILD_ENV` and
the Rust target partition incompatible environments before fingerprinting.
`SK_NATIVE_SDK` bypasses the shared resolver for deliberately custom SDKs.

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

For the Vespa SDK, `tools/resolve-native-sdk.sh` first computes the same
provenance fingerprint that will be recorded in the package. The digest is the
cache object identity. A matching object is reused read-only; a miss acquires a
per-digest lock, invokes `core/native/vespa/build-sdk.sh` into private staging,
verifies the resulting digest, marks the package read-only, and atomically
publishes it. The Rust bindings still compile/link a probe that calls
`sk_native_abi_version()` from the actual SDK library, including its bundled
transitive runtime libraries.

A probe failure never recompiles inside a published cache object. Published
objects are immutable: if an existing object fails validation or the link probe,
the build stops with a cache-integrity error rather than allowing one worktree
to rewrite an object consumed by other worktrees. Native source changes cannot
overwrite another worktree's object because `native.source` is part of the
fingerprint and therefore selects a different key.

The configured Vespa CMake outputs themselves remain intentionally untracked.
When the current worktree does not contain them, the resolver scans sibling Git
worktrees and may use a configured `upstream/vespa` only when its pinned Vespa
identity matches the requesting worktree. Thus ordinary Rust-only agent
worktrees do not need their own Vespa build tree.

Neither native path runs `cargo clean` or deletes the entire target directory.
If no matching sibling contains configured Vespa outputs and no cache object can
be resolved, the resolver reports that prerequisite directly.

Useful verification commands are:

```bash
bash tools/test-native-environment-fingerprint.sh
bash tools/test-native-build-hardening.sh
bash tools/test-native-sdk-cache.sh
cargo test --workspace --all-targets --no-default-features
```

The first verifier exercises stable fingerprints, compiler-wrapper changes,
flag changes, and host/container/CI target separation. The second verifies
FlatBuffers warm reuse, targeted invalidation, preservation of unrelated Rust
artifacts, and one-shot recovery from a broken archive.


## Worktree isolation invariant

The cache is shared storage, not shared mutable state.

For any two worktrees:

```text
identical native provenance
    -> identical cache key
    -> safe reuse of one read-only SDK object

different native provenance
    -> different cache key
    -> independent SDK object
```

In particular, edits outside the native fingerprint (for example IR, protocol,
client, or filter changes) do not force a Vespa rebuild. Edits under
`core/native/vespa`, a different pinned/configured Vespa tree, toolchain or
runtime changes, target changes, and relevant native flags do.
