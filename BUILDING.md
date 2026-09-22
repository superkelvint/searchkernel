# Building Search Kernel

Search Kernel builds Rust code against pinned native prerequisites. FlatBuffers
C++ 25.12.19 is installed once as a clone-wide toolchain and is never rebuilt by
ordinary Cargo commands. The default native feature links a small Search Kernel-owned Vespa adapter
against an immutable pinned Vespa/runtime base. Cargo build outputs, including
the adapter, remain environment-local; third-party native prerequisites are
shared safely across Git worktrees.

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
`.cargo/config.toml`. Ordinary `./dev check-fast`, standalone Rust API
tests, and non-native workspace checks do not populate the Vespa SDK cache.
Before the first native link (and after a declared Vespa-base input changes),
bootstrap or repair the immutable base explicitly with `./dev native-sdk ensure`.
Ordinary Cargo builds resolve that base through the read-only `path` lookup and
build only Search Kernel's `abi.cc` adapter into the Cargo target's `OUT_DIR`.
If the base is missing or incompatible, Cargo fails with the
`./dev native-sdk ensure` remediation; Cargo never builds or repairs the shared
Vespa base. `./dev native-sdk path` is diagnostic-only and never mutates it.

A fresh or isolated worktree is not expected to already contain a built
`searchkerneld` binary. If `./dev doctor` reports it as informational,
build it with `cargo build -p searchkerneld`.


```bash
cargo build -p searchkerneld
cargo test --workspace --all-targets --no-default-features
```

`searchkerneld` enables the Vespa native feature by default. Its Rust bindings
resolve a read-only shared Vespa base, build the Search Kernel adapter locally,
and run a real compile/link probe against the combined adapter + base before Rust
links. Search Kernel source changes never rebuild or repackage the Vespa base.

The base cache is stored below the repository's Git common directory, so all
worktrees from one clone can reuse matching objects without sharing mutable Cargo
build directories. Published base objects contain Vespa/runtime libraries only:
no `libsearchkernel_vespa.so` and no Search Kernel ABI header. The base identity
tracks the pinned Vespa/config identity plus the actual Vespa, dependency, and
runtime library bytes that will be published, partitioned by target/environment.
The active Rust/C++ compiler, adapter source, failpoints, sanitizer, coverage,
FlatBuffers version, and adapter-only flags do not change the base key.

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
outputs. The Vespa base resolver separately partitions its shared object store by
`SEARCHKERNEL_BUILD_ENV` and target, then keys base objects by base-scope
provenance. `SK_NATIVE_SDK` bypasses this split model for a deliberately
managed full SDK; full-package provenance still includes Search Kernel adapter
source and adapter-only native flags.

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

Cargo's protocol build uses this same installed FlatBuffers C++ prerequisite.
It verifies the exact pinned header version and static library, then compiles
only Search Kernel's small JSON bridge against them. Cargo never downloads,
configures, compiles, invalidates, or repairs FlatBuffers itself.

A fresh Cargo target directory or Git worktree therefore does not rebuild
FlatBuffers. If the prerequisite is missing or the wrong version, the build
fails quickly and tells the developer to run `./dev install flatbuffers`.
Only an intentional FlatBuffers version/toolchain change requires another
FlatBuffers installation.

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

## Vespa base and adapter builds

Bootstrap or repair the immutable Vespa base explicitly before the first native
build (or after a declared base-input change):

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
dependency installation expected by `build-sdk.sh`. The explicit `./dev native-sdk ensure` bootstrap builds
only the immutable Vespa/runtime base, serializes builders per base fingerprint,
validates runtime closure, marks the completed base read-only, and publishes it
atomically. Ordinary Cargo then compiles/relinks only Search Kernel's adapter
against that base. A direct `build-sdk.sh` invocation still produces the full
self-contained release/qualification SDK, with full-package provenance including
adapter source and flags.

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

`./dev doctor` does not build native artifacts. Native SDK / ABI / pinned
Vespa readiness is informational for ordinary non-native development and becomes
a prerequisite only when the requested verification actually uses the native
feature.

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

The native verifiers cover host/container/CI isolation, preservation of
unrelated Rust artifacts, and the invariant that ordinary Cargo builds consume
the installed FlatBuffers prerequisite without rebuilding it.

Do not use `cargo clean`, delete `target/`, or manually remove an SDK as a
normal response to a native linker failure. Read the first provenance warning
or prerequisite error; the affected native component is designed to rebuild
itself once.

Do not hand-edit generated SDK contents.

For the complete fingerprint fields and recovery state machine, see
[`docs/native-build-provenance.md`](docs/native-build-provenance.md).
