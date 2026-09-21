# Building Search Kernel

Search Kernel builds Rust code together with two native components: the
vendored FlatBuffers C++ library and, for the default native feature, the
Vespa adapter SDK. Native outputs are compiler- and runtime-sensitive, so the
repository selects an environment-specific Cargo target root and validates
native provenance before linking.

## Normal development

Run the prerequisite check first when setting up a checkout or diagnosing an
environment failure:

```bash
./dev doctor
```

The normal host workflow uses `target/host` automatically through
`.cargo/config.toml`:

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

The no-default-features workspace command exercises the Rust and FlatBuffers
paths without requiring the optional Vespa SDK:

```bash
cargo test --workspace --all-targets --no-default-features
```

## Environment isolation

Supported workflows use these independent target roots:

| Environment | Target root | Native SDK |
| --- | --- | --- |
| Host | `target/host` | `target/host/native-sdk/linux-x86_64` |
| Project container | `target/devcontainer` | `target/devcontainer/native-sdk/linux-x86_64` |
| CI | `target/ci` | `target/ci/native-sdk/linux-x86_64` |

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

`CARGO_TARGET_DIR` is the isolation boundary. `SK_NATIVE_SDK` is available for
an explicitly managed SDK outside the default target root, but it must point to
an SDK built for the same compiler, target, libc, C++ runtime, Vespa source,
and native flags as the consuming build.

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

Normal `cargo build -p searchkerneld` is the preferred entry point. A direct
SDK build is useful for native adapter work, qualification, or a configured
Vespa checkout with explicit dependency paths:

```bash
VESPA_ROOT="$PWD/upstream/vespa" \
VESPA_DEPS=/path/to/vespa-deps \
SK_NATIVE_SDK="$PWD/target/host/native-sdk/linux-x86_64" \
SK_NATIVE_BUILD_DIR="$PWD/target/host/native-sdk-build" \
  core/native/vespa/build-sdk.sh
```

The pinned Vespa checkout must already have the configured CMake outputs and
dependency installation expected by `build-sdk.sh`. The script stages and
publishes the SDK atomically, writes its fingerprint and manifest, and never
requires deleting the whole target directory. Use a separate SDK/build path
for qualification or sanitizer configurations.

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
