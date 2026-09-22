# Native build provenance

For day-to-day commands and prerequisite handling, start with
[`BUILDING.md`](../BUILDING.md). This document records the provenance model
that makes those commands safe against warm native caches.

Search Kernel separates immutable third-party prerequisites from project build
artifacts:

```text
Git common directory
  ├── searchkernel/toolchains/flatbuffers/25.12.19/
  │      immutable installed FlatBuffers C++ toolchain
  └── searchkernel/native-sdk-cache/
         └── <environment>/<target>/objects/<fingerprint>/
                                               immutable Vespa/runtime base
```

Ordinary Cargo targets contain Search Kernel build artifacts only. FlatBuffers
is installed once per clone/version and is read by every worktree; Cargo never
owns or repairs a per-target FlatBuffers C++ build.

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

Native provenance has two explicit scopes. The shared Vespa base records only
inputs that can change the pinned third-party prerequisite. Full/package
provenance additionally records Search Kernel adapter source and adapter-only
flags.

- Rust host/toolchain and Cargo target;
- canonical C/C++ compiler paths, versions, and target/dumpmachine output;
- archiver, ranlib, linker, libc, and C++ runtime identities;
- relevant compiler, linker, CMake, and standard-library configuration for the
  base;
- for full/package scope only: Search Kernel adapter source plus failpoint,
  sanitizer, coverage, and adapter-only compiler/linker flags;
- the pinned FlatBuffers version consumed by native packaging where relevant;
- the pinned Vespa revision and configured Vespa build inputs; and
- the selected dependency/runtime library content bundled into the base.

The record contains no timestamps, process IDs, temporary paths, or random
values. The native SDK also stores its fingerprint digest in `manifest.json`.

## Build behavior

FlatBuffers has no Cargo-native recovery state machine. The protocol build
resolves the clone-wide/system FlatBuffers 25.12.19 installation, verifies its
headers and static archive, and either consumes it read-only or fails with the
one-time bootstrap command.

For the Vespa base, `tools/resolve-native-sdk.sh` computes a base-scope
fingerprint. Cache objects contain the Vespa/runtime closure only and explicitly
exclude Search Kernel adapter libraries and headers. Matching bases are reused
read-only only after their runtime dependency closure validates. Cargo builds
Search Kernel's adapter separately in `OUT_DIR` and probes the adapter against
the immutable base.

A cache miss acquires a per-digest lock, invokes
`core/native/vespa/build-sdk.sh` into private staging, validates the resulting
digest and runtime closure, marks the package read-only, and atomically
publishes it. If a previously published object fails validation, the resolver
rechecks it under the same lock, moves it to quarantine, rebuilds privately, and
republishes a valid object at that key. No consumer mutates or recompiles inside
a published object.

The Rust bindings compile/link a probe that calls `sk_native_abi_version()`
through the local adapter while resolving its dependencies from the shared base.
Adapter source changes rebuild only that local adapter. Base dependency/runtime
changes select a new base key. Direct full SDK packaging uses full-scope
provenance so stale adapter implementations remain detectable.

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
flag changes, and host/container/CI target separation. The second verifies that
Cargo never rebuilds FlatBuffers, including from fresh target directories, while
preserving unrelated Rust artifacts.


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
client, or filter changes) do not force a Vespa rebuild. Edits to Search Kernel adapter sources do not change the shared base key; they
rebuild only the local adapter. A different pinned/configured Vespa tree,
base-relevant toolchain/runtime change, target change, or packaged dependency
change selects a different base object.
