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
                                               immutable Vespa adapter SDK
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

The Vespa native SDK records a deterministic provenance fingerprint containing
the inputs that can affect binary compatibility, including:

- Rust host/toolchain and Cargo target;
- canonical C/C++ compiler paths, versions, and target/dumpmachine output;
- archiver, ranlib, linker, libc, and C++ runtime identities;
- relevant compiler, linker, CMake, standard-library, sanitizer, coverage, and
  native configuration flags;
- the pinned FlatBuffers version consumed by native packaging where relevant;
- the pinned Vespa revision and configured Vespa build inputs for the SDK; and
- the Search Kernel native source/header/configuration content.

The record contains no timestamps, process IDs, temporary paths, or random
values. The native SDK also stores its fingerprint digest in `manifest.json`.

## Build behavior

FlatBuffers has no Cargo-native recovery state machine. The protocol build
resolves the clone-wide/system FlatBuffers 25.12.19 installation, verifies its
headers and static archive, and either consumes it read-only or fails with the
one-time bootstrap command.

For the Vespa SDK, `tools/resolve-native-sdk.sh` first computes the same
provenance fingerprint that will be recorded in the package. The digest is the
cache object identity. The fingerprint includes Search Kernel native/package
source, configured Vespa identity/build outputs, content identity for the
selected external dependency/runtime shared libraries that are bundled into the
SDK, and the native toolchain/configuration inputs. A matching object is reused
read-only only after its runtime dependency closure validates.

A cache miss acquires a per-digest lock, invokes
`core/native/vespa/build-sdk.sh` into private staging, validates the resulting
digest and runtime closure, marks the package read-only, and atomically
publishes it. If a previously published object fails validation, the resolver
rechecks it under the same lock, moves it to quarantine, rebuilds privately, and
republishes a valid object at that key. No consumer mutates or recompiles inside
a published object.

The Rust bindings still compile/link a probe that calls
`sk_native_abi_version()` from the actual SDK library as an independent final
consumer check. Native source/package changes and changed packaged dependency
libraries select a different cache key; accidental corruption of an existing
object self-heals under the per-key lock.

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
client, or filter changes) do not force a Vespa rebuild. Edits under
`core/native/vespa`, a different pinned/configured Vespa tree, toolchain or
runtime changes, target changes, and relevant native flags do.
