# Building Search Kernel

Search Kernel builds Rust code together with two native components: the
vendored FlatBuffers C++ library and, for the default native feature, the
Vespa adapter SDK. Native outputs are compiler- and runtime-sensitive. Cargo
build outputs remain environment-local, while Vespa SDK packages are resolved
through an immutable content-addressed store shared by Git worktrees and keyed
by the full native provenance fingerprint.

## Authority and failure handling

This file is the authoritative guide for building, compiling, toolchain setup,
native SDK acquisition, standard Rust verification commands, and
environment/build recovery.

Agents and developers MUST read this file before their first build or
verification run in an unfamiliar checkout. If a build, compiler, linker,
FlatBuffers, Vespa SDK, dependency, cache, or provenance error occurs, treat it
as an environment/build problem first:

1. run `./dev doctor`;
2. fix or report each failed prerequisite;
3. follow the component-specific recovery below;
4. rerun the original command;
5. only after the documented prerequisites are healthy, investigate product
   code as the cause.

Do not edit production code merely to work around a broken build environment.
Do not use `cargo clean`, delete `target/`, copy SDK directories between
environments, or rebuild immutable SDK cache objects as an ad-hoc repair.

## Repository command surface

Use the root `./dev` command as the normal entry point for repository setup,
verification, generation, and acceptance lanes:

```text
./dev doctor
./dev check-fast
./dev test ir
./dev test rust-api
./dev test python
./dev regen protocol
./dev verify m13
./dev verify integration
./dev install flatbuffers
./dev native-sdk ensure
./dev native-sdk path
```

Use `./dev --help` for the supported command list. Direct underlying scripts
are for focused debugging or when a contract explicitly names one.

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

A fresh or isolated worktree is not expected to already contain a built
`searchkerneld` binary. If `./dev doctor` reports it as informational,
build it with `cargo build -p searchkerneld`.


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
for standalone schema tooling and doctor checks. The pinned version is
`25.12.19`. By default the install is clone-wide and versioned under the Git
common directory:

```text
<git-common-dir>/searchkernel/toolchains/flatbuffers/25.12.19/
```

All worktrees in the clone resolve that installation automatically, so an
agent worktree must not download or rebuild its own `flatc` merely because its
local `target/` is empty. The resolver also reuses a matching `flatc` already
built in a sibling worktree before falling back to `/usr/local`.

Run `./dev install flatbuffers` once per clone/version when no matching toolchain
exists. Concurrent installs are serialized and a healthy existing install is
reused without rebuilding. For a deliberate custom prefix, set
`FLATBUFFERS_INSTALL_PREFIX`; explicit `FLATBUFFERS_CPP_ROOT` and `FLATC_PATH`
overrides remain supported.

Cargo's protocol build still uses its matching pinned vendored FlatBuffers
build under the active target root. The shared standalone toolchain is for
schema generation, reproducibility checks, and doctor; it is not silently
mixed into Cargo's native build outputs.

The FlatBuffers build records a deterministic fingerprint and compiles a
parser/link probe against the actual static archive. A changed fingerprint or
failed probe invalidates only that FlatBuffers sub-build. Rust incremental
outputs and unrelated native components remain intact.

## Python runtime vs code-generation dependencies

Baseline Python runtime checks require the dependencies needed to import and
use the checked-in client/runtime code. `flatbuffers==25.12.19` and
`grpcio==1.84.0` are exact runtime pins. Protobuf follows the package's
declared compatibility range, `protobuf>=7.35.1,<8`; a newer compatible 7.x
runtime such as 7.36.2 is healthy and must not fail `./dev doctor`.

`grpcio-tools` (`grpc_tools`) is a code-generation dependency only. Generated
gRPC sources remain reproducible by requiring the exact codegen stack:
`grpcio-tools==1.84.0` and `protobuf==7.35.1`. These exact codegen pins do
not narrow the Python client's runtime protobuf compatibility range.
`grpcio-tools` is required by `./dev regen protocol` and
`tools/check-python-grpc.sh`, but its absence does not make ordinary
`./dev doctor`, Rust builds, or SDK-independent verification unhealthy.
Install the Python `codegen` extra only when changing or verifying generated
gRPC sources:

```bash
python3 -m pip install -e 'clients/python[codegen]'
```

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

## Native SDK environment recovery

Normal development should not require native SDK environment overrides.

Use:

```bash
./dev doctor
./dev native-sdk ensure
```

The resolver discovers a configured pinned Vespa build from the current or a
matching sibling worktree, discovers its repository-local `vespa-deps`, and
lets the SDK builder discover the matching LLVM/libffi runtime from the same
clone. The resulting dependency/runtime inputs are part of the native cache
identity.

A published SDK object is reused only after its bundled runtime dependency
closure validates. If an otherwise matching immutable cache object is incomplete
or corrupt, the resolver acquires that digest's cache lock, moves the bad object
to quarantine for diagnosis, rebuilds into private staging, validates the new
SDK, and atomically republishes it. Consumers never rebuild or mutate a published
object in place.

Therefore an agent encountering missing `libvespa_*.so`, LLVM, or other SDK
runtime libraries should **not** start by exporting `VESPA_DEPS`,
`VESPA_DEPS_LIB_DIR`, `VESPA_EXTRA_LIB_DIR`, deleting cache objects, or
running `cargo clean`. Run `./dev doctor` or `./dev native-sdk ensure` and
let the resolver recover the canonical environment.

Environment variables such as `VESPA_ROOT`, `VESPA_DEPS`,
`VESPA_DEPS_LIB_DIR`, `VESPA_EXTRA_LIB_DIR`, and `VESPA_AWS_LIB_DIR`
remain supported only for deliberately custom/non-standard environments or when
automatic discovery reports that no unique usable input exists. When such an
override is genuinely necessary, the diagnostic should identify the missing
input explicitly.

After recovery, rerun the original Cargo command. Cargo's native ABI/link probe
remains an independent final check that the resolved SDK is consumable.

## Task worktrees and native prerequisites

`./dev worktree create <task-name>` creates a temporary worktree at
`/tmp/<repository>-<task-name>` on `agent/<task-name>` from the current
`HEAD`; it does not copy uncommitted changes.

Temporary worktrees also do not copy the pinned, read-only Vespa checkout. When
local verification needs the canonical checkout, link the shared read-only
inputs into the worktree:

```bash
mkdir -p /tmp/searchkern2-<task-name>/upstream
ln -s /home/kelvin/projects/searchkern2/upstream/vespa \
  /tmp/searchkern2-<task-name>/upstream/vespa
ln -s /home/kelvin/projects/searchkern2/lib64 \
  /tmp/searchkern2-<task-name>/lib64
```

If a destination already exists, verify that it points to the canonical shared
directory instead of replacing it. Do not modify or commit the shared
read-only directories from a task worktree.

## Standard Rust verification

For a focused Rust change, normally run the affected crate's:

```text
cargo fmt --check
cargo clippy -p <crate> --all-targets --all-features -- -D warnings
cargo test -p <crate> --all-targets --all-features
```

For changes crossing crate/API/IR/native boundaries or shared infrastructure,
run the full workspace checks:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Use the no-default-features lane when intentionally verifying Rust/FlatBuffers
without the optional Vespa SDK:

```text
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --all-targets --no-default-features
```

There must be zero compiler and Clippy warnings.

## Native build verification


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

Do not hand-edit generated SDK contents.

For the complete fingerprint fields and recovery state machine, see
[`docs/native-build-provenance.md`](docs/native-build-provenance.md).
