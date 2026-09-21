# Building Search Kernel

Search Kernel builds Rust code together with two native components: the
vendored FlatBuffers C++ library and, for the default native feature, the
Vespa adapter SDK. Native outputs are compiler- and runtime-sensitive. Cargo
build outputs remain environment-local, while Vespa SDK packages are resolved
through an immutable content-addressed store shared by Git worktrees and keyed
by the full native provenance fingerprint.

## Normal development

Run the prerequisite check first when setting up a checkout or diagnosing an
environment failure:

```bash
./dev doctor
```

The normal host workflow uses `target/host` automatically through
`.cargo/config.toml`. Before linking a native target, Cargo resolves the
matching shared SDK automatically; `./dev native-sdk ensure` may be used to
populate it explicitly:

```bash
cargo build -p searchkerneld
cargo test --workspace --all-targets --no-default-features
```

`searchkerneld` enables the Vespa native feature by default. Its Rust bindings
derive the SDK location from the active Cargo target directory, validate the
SDK fingerprint, and run a real ABI link probe before Rust links. If the SDK is
missing, stale, or incompatible, Cargo invokes one targeted SDK rebuild and
tries the probe once more. A second failure is reported with the prerequisite,
compiler, target, and linker diagnostics that need attention.

The cache is stored below the repository's Git common directory, so all
worktrees from one clone can reuse matching SDK objects without sharing mutable
build directories. Published objects are read-only. A different native source,
Vespa/config identity, target, compiler/runtime, or relevant native flag yields
a different fingerprint and therefore a different object.

The no-default-features workspace command exercises the Rust and FlatBuffers
paths without requiring the optional Vespa SDK:

```bash
cargo test --workspace --all-targets --no-default-features
```

## Environment isolation

Supported workflows use these independent target roots:

| Environment | Cargo target root | Vespa SDK namespace |
| --- | --- | --- |
| Host | `target/host` | `<git-common-dir>/searchkernel/native-sdk-cache/host/<target>/objects/<fingerprint>` |
| Project container | `target/devcontainer` | `.../native-sdk-cache/devcontainer/<target>/objects/<fingerprint>` |
| CI | `target/ci` | `.../native-sdk-cache/ci/<target>/objects/<fingerprint>` |

The project container sets `CARGO_TARGET_DIR=target/devcontainer`. The CI ABI
workflow sets `CARGO_TARGET_DIR=target/ci`. Do not manually mount or restore a
host `target/host` directory into the container, or restore native output from
one CI image/toolchain into another.

For a deliberate custom environment, choose a new target root for the entire
build:

```bash
CARGO_TARGET_DIR=target/my-native-environment \
SEARCHKERNEL_BUILD_ENV=my-native-environment \
  cargo build -p searchkerneld
```

`CARGO_TARGET_DIR` remains the isolation boundary for Cargo/FlatBuffers build
outputs. The Vespa SDK resolver separately partitions its shared object store by
`SEARCHKERNEL_BUILD_ENV` and target, then keys objects by the complete native
fingerprint. `SK_NATIVE_SDK` bypasses this resolver for an explicitly managed
SDK and must point to an SDK built for the same compiler, target, libc, C++
runtime, Vespa source, and native flags as the consuming build.

## FlatBuffers

`./dev install flatbuffers` installs the pinned official FlatBuffers C++ tools
for standalone schema tooling and doctor checks. Cargo's protocol build uses
its matching pinned vendored FlatBuffers build under the active target root;
the installed `/usr/local` copy is not silently mixed with Cargo's headers or
archive.

The FlatBuffers build records a deterministic fingerprint and compiles a
parser/link probe against the actual static archive. A changed fingerprint or
failed probe invalidates only that FlatBuffers sub-build. Rust incremental
outputs and unrelated native components remain intact.

## Vespa SDK builds

Normal `cargo build -p searchkerneld` is the preferred entry point. The
equivalent explicit cache commands are:

```bash
./dev native-sdk ensure
./dev native-sdk path
```

If the current worktree lacks untracked configured Vespa build outputs, the
resolver searches sibling Git worktrees for a configured `upstream/vespa`
with the same pinned Vespa identity. This is what allows an isolated agent
worktree to run all-features Clippy/tests without copying the large Vespa build
tree.

A direct SDK build is still useful for native adapter qualification or an
explicitly managed SDK:

```bash
VESPA_ROOT="$PWD/upstream/vespa" \
VESPA_DEPS=/path/to/vespa-deps \
SK_NATIVE_SDK="$PWD/target/host/native-sdk/linux-x86_64" \
SK_NATIVE_BUILD_DIR="$PWD/target/host/native-sdk-build" \
  core/native/vespa/build-sdk.sh
```

The selected Vespa checkout must have the configured CMake outputs and
dependency installation expected by `build-sdk.sh`. The shared resolver
serializes builders per fingerprint, builds into a private staging directory,
verifies the resulting fingerprint, marks the completed SDK read-only, and
publishes it atomically. It never builds directly inside an object visible to
another worktree. Use a separate SDK/build path for qualification or sanitizer
configurations when bypassing the resolver.

When a task worktree does not contain `vespa-deps`, the script looks for a
repository-local `vespa-deps` in a sibling Git worktree before falling back to
the absolute path recorded by the Vespa build. Passing `VESPA_DEPS` explicitly
is still preferred when the dependency installation belongs to a different
container or host.

## Verification

Run the focused native provenance checks and the normal workspace checks:

```bash
bash tools/test-native-environment-fingerprint.sh
bash tools/test-native-build-hardening.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --all-targets --no-default-features
```

The native verifiers cover stable warm reuse, compiler-wrapper and flag
invalidation, host/container/CI target separation, preservation of unrelated
Rust artifacts, and one-shot recovery from a broken FlatBuffers archive.

Do not use `cargo clean`, delete `target/`, or manually remove an SDK as a
normal response to a native linker failure. Read the first provenance warning
or prerequisite error; the affected native component is designed to rebuild
itself once.

For the complete fingerprint fields and recovery state machine, see
[`docs/native-build-provenance.md`](docs/native-build-provenance.md).
