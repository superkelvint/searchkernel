# Building Search Kernel

Search Kernel builds Rust code against pinned native prerequisites. FlatBuffers
C++ 25.12.19 is installed once as a clone-wide toolchain and is never rebuilt by
ordinary Cargo commands. The default native feature links a small Search Kernel-owned Vespa adapter
against an immutable pinned Vespa/runtime base. Cargo build outputs, including
the adapter, remain environment-local; the Vespa base is installed once in a
machine-global cache and reused by independent clones and worktrees.

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
./dev verify m14
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

The base cache is machine-global by default under
`${XDG_CACHE_HOME:-$HOME/.cache}/searchkernel/native-sdk-cache`, so independent
Git clones and worktrees on the same machine reuse the same immutable base.
Published base objects contain Vespa/runtime libraries only: no
`libsearchkernel_vespa.so` and no Search Kernel ABI header. The machine-global cache is partitioned by declared build environment and
native target. Within that compatibility namespace, the object key is exactly
the pinned Vespa checkout hash from `core/native/vespa/manifest.json`. There is
no second base fingerprint derived from DSO bytes, dependency/runtime bytes,
compiler state, adapter source, or per-clone state.

The no-default-features workspace command exercises the Rust and FlatBuffers
paths without requiring the optional Vespa SDK:

```bash
cargo test --workspace --all-targets --no-default-features
```

### CI does not require Vespa source

Ordinary GitHub CI uses the non-native lanes and must not depend on an
`upstream/vespa` checkout or call `./dev native-sdk ensure`. Native-sensitive
pull requests are still classified so reviewers can see that native behavior may
be affected, but the real-engine acceptance command is run locally by the
implementation agent for now.

When a change affects the native adapter, bindings, Vespa engine, IR adapter,
native build tooling, or real-Vespa acceptance artifacts, run the applicable
native verifier locally and include the exact command and result in the pull
request. Do not interpret a green non-native CI run as native verification.

The repository-managed pre-commit hook enforces this split. When any checked-in
native ABI authority is staged, it first checks the Git index with
`tools/check-native-abi-authorities.py --git-index`, so a version bump cannot
commit with stale manifests, Rust constants, reference contracts, or pinned
metadata. Staged native-sensitive changes then run both the real-Vespa gate and
the real Python gRPC E2E through `tools/run-python-native-e2e.sh`. That runner opts the native E2E test in explicitly
and bootstraps the native SDK first. Non-native-only Rust or Python changes do not
trigger native verification solely because their language lane is selected.
Ordinary Python unittest discovery leaves the real gRPC E2E skipped, so GitHub CI
does not acquire or build Vespa merely because Python behavioral tests are selected.


## Environment isolation

Supported workflows use these independent target roots:

| Environment | Cargo target root | Vespa SDK namespace |
| --- | --- | --- |
| Host | `target/host` | `<machine-cache>/searchkernel/native-sdk-cache/host/<target>/objects/<vespa-checkout-hash>` |
| Project container | `target/devcontainer` | `<machine-cache>/searchkernel/native-sdk-cache/devcontainer/<target>/objects/<vespa-checkout-hash>` |
| CI | `target/ci` | `<machine-cache>/searchkernel/native-sdk-cache/ci/<target>/objects/<vespa-checkout-hash>` |

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
outputs. The Vespa base resolver separately partitions its machine-global store
by `SEARCHKERNEL_BUILD_ENV` and native target, then addresses the immutable
object inside that compatibility namespace by the pinned Vespa checkout hash.
Compiler identity and Search Kernel adapter source/flags do not create another
Vespa-base key. `SK_NATIVE_SDK` bypasses this split
model for a deliberately managed full SDK; full-package provenance still
includes Search Kernel adapter source and adapter-only native flags.

## FlatBuffers

`./dev install flatbuffers` installs the pinned official FlatBuffers C++ tools
for standalone schema tooling and doctor checks. The pinned version is
`25.12.19`. By default the install is clone-wide and versioned under the Git
common directory:

```text
<git-common-dir>/searchkernel/toolchains/flatbuffers/25.12.19/<build-environment>/
```

All worktrees in the clone resolve the installation for their build environment
automatically, so an agent worktree must not download or rebuild its own
`flatc` merely because its local `target/` is empty. Host, project-container,
and other explicitly named `SEARCHKERNEL_BUILD_ENV` environments are kept
separate because the installed C++ static library is ABI-sensitive. The resolver
also reuses a matching `flatc` already built in the same environment before
falling back to `/usr/local`.

Run `./dev install flatbuffers` once per clone/version/build-environment when
no matching toolchain exists. Concurrent installs are serialized and a healthy
existing install is reused without rebuilding. Reuse requires an actual C++
compile/link/runtime probe against the installed headers and
`libflatbuffers.a`; version-correct files that were built against an
incompatible libc/C++ environment are rejected and rebuilt for the current
environment instead of being allowed to fail later during Cargo linking.
`./dev doctor` runs the same probe, so a FlatBuffers C++ prerequisite is green
only when it is usable by the active compiler/runtime. For a deliberate custom
prefix, set
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

The resolver first checks the machine-global object for the pinned Vespa checkout
hash and the machine-global registration of the one configured checkout with that
same hash. A fresh independent Git clone therefore reuses the installed base and
the already-configured immutable checkout; it does not need to clone, configure,
or build Vespa itself.

Only when provisioning or repairing a genuinely missing/corrupt machine base
does `./dev native-sdk ensure` use the configured `upstream/vespa` checkout
or a compatible pre-split full SDK. A standalone provisioning checkout must have
Git HEAD equal to the pinned Vespa checkout hash. If the configured checkout has
the required CMake metadata but its `libvespa*.so*` outputs are absent, `ensure`
builds the configured `searchcore_embedded_crud_spike_app` target once, then
packages the resulting Vespa/runtime closure into the immutable base. `ensure`
registers that checkout path once in the machine-global cache for compatible
clones on the machine. A compatible pre-split full SDK is accepted when its
manifest records that same checkout hash.

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
only the immutable Vespa/runtime base, serializes provisioning per Vespa checkout
hash, validates runtime closure, marks the completed base read-only, and publishes
it atomically. Ordinary Cargo then compiles/relinks only Search Kernel's adapter
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

The resolver first checks the machine-global base and registered toolchain
keyed by the checked-in Vespa checkout hash. Normal agent clones therefore need
no clone-local Vespa tree when that machine has already been provisioned.

On a cache miss, `ensure` prefers a compatible already-built local source. If
none exists, it acquires the canonical published immutable base for the current
build-environment/native-target/Vespa-pin namespace before considering a local
Vespa build. The default artifact is a GitHub release asset named
`searchkernel-vespa-base-v1-<environment>-<target>-<vespa-commit>.tar.gz`;
`SK_NATIVE_BASE_ARTIFACT_URL` may point at an explicitly managed mirror. The
download is checksum-verified, its manifest must exactly match the environment,
target and pin, and unsafe archive entries are rejected before publication.

The published artifact contains the immutable Vespa/runtime base plus the
read-only Vespa header/config/dependency-header toolchain required to compile
Search Kernel's local `abi.cc` adapter. It never contains the Search Kernel
adapter itself.
`tools/package-native-base-artifact.sh` creates this bundle, and
`.github/workflows/publish-native-base.yml` publishes the canonical host asset
from `main`. Only when the published artifact is unavailable may an
already-configured pinned Vespa checkout be used as the explicit last-resort
bootstrap; if its DSOs are absent, `ensure` may build the configured
`searchcore_embedded_crud_spike_app` target once. A compatible pre-split full
SDK remains a faster local seed when available.

A published SDK object is reused only after its bundled runtime dependency
closure validates. If the checkout-hash object is incomplete or corrupt, the
resolver acquires that checkout's lock, moves the bad object to quarantine for
diagnosis, rebuilds into private staging, validates the new SDK, and atomically
republishes it. Consumers never rebuild or mutate a published
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

Temporary worktrees and independent clones do not need their own pinned Vespa
checkout when the machine has been provisioned for the same build environment
and native target. `./dev native-sdk path`
resolves the shared base, and the native adapter build resolves the one registered
configured checkout for the same pinned hash.

Configure Vespa only in the one provisioning checkout (or when intentionally
performing Vespa/native qualification work). Do not copy or clone Vespa into
each task checkout.

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
