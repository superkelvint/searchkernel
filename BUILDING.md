# Building Search Kernel

> **Consumer note:** the native SDK/cache controls below are contributor tooling. Supported consumer builds automatically provision the pinned native runtime; consumers should not set `SK_NATIVE_SDK` or pre-populate the native cache.


Search Kernel builds Rust code against pinned native prerequisites. FlatBuffers
C++ 25.12.19 is installed once as a clone-wide toolchain and is never rebuilt by
ordinary Cargo commands. The default native feature links a small Search Kernel-owned Vespa adapter
against an immutable pinned Vespa/runtime base. Cargo build outputs, including
the adapter, remain environment-local; the Vespa base is installed once in a
machine-global cache and reused by independent clones and worktrees.


## Fresh developer machine

For a new Linux developer machine, the supported path is intentionally small:
install the host bootstrap tools, authenticate GitHub, clone the repository, and
let `./dev doctor` repair and verify the rest. Normal repository verification
runs in the canonical `searchkernel-ci` Docker image, so developers do not need
to manually reproduce CI's Ruby, PHP, Node, GCC, FlatBuffers, or Vespa
toolchains on the host.

### 1. Install host bootstrap prerequisites

The host needs:

- Git and `curl`;
- Docker Engine, usable by the developer without `sudo`;
- GitHub CLI (`gh`);
- Python 3.10 or newer with `venv`;
- `rustup`;
- a basic C/C++ build environment and CMake.

On Ubuntu/Debian, a suitable baseline is:

```bash
sudo apt-get update
sudo apt-get install -y \
  git curl ca-certificates \
  build-essential cmake \
  python3 python3-venv python3-pip \
  clang libclang-dev python3-clang \
  ripgrep
```

Install Docker Engine and GitHub CLI through their supported distribution
packages, then install Rust through `rustup`. Do not install the language/runtime
matrix used by CI manually; the canonical Docker image owns those versions.

Verify the bootstrap tools before cloning:

```bash
docker version
docker run --rm hello-world

gh auth login
gh auth status

python3 --version
rustup --version
```

The authenticated GitHub account must have access to the private
`superkelvint/searchkernel` repository. A normal developer shell does not need
to export `GH_TOKEN` or `GITHUB_TOKEN`; native-base discovery and private
release downloads reuse the authenticated `gh` session when explicit token
variables are absent.

### 2. Clone and bootstrap the repository

```bash
gh repo clone superkelvint/searchkernel
cd searchkernel

./dev doctor
```

`./dev doctor` is the authoritative first-run bootstrap and recovery command.
It repairs or verifies the host Rust/Node/Python/system tooling, installs the
pinned FlatBuffers prerequisite when needed, installs repository Python and
TypeScript dependencies, provisions the host native prerequisite, configures the
repository hooks, verifies generated protocol artifacts, and performs a real
native `searchkerneld` build.

A successful doctor means the host compile/link path is usable, not merely that
similarly named executables exist on `PATH`.

### 3. Validate the canonical Docker-backed native environment

Normal build/test verification uses the same immutable environment as CI.
Validate that environment explicitly once after bootstrap:

```bash
./dev native-sdk ensure
```

This command runs through the normal CI-image path and validates the exact
digest-addressed Vespa/native bundle referenced by
`docker.io/superkelvint/searchkernel-ci:latest`. It does not create a second
host-native cache.

### 4. Build

For an explicit host build:

```bash
cargo build -p searchkerneld
```

The normal host target root is `target/host`; the daemon is therefore normally
written to:

```text
target/host/debug/searchkerneld
```

The host build is useful for direct daemon development. Repository acceptance
checks should still use `./dev`, because those checks intentionally execute in
the canonical Docker environment.

### 5. Run the test gates

Run these in increasing order of scope:

```bash
# Fast development gate.
./dev check-fast

# Full normal repository quality/test gate.
./dev check

# Broad routine checks (same suite as ./dev check, without Universe-only lanes).
./dev galaxy

# Complete qualification: clients, E2E lanes, native/semantic verifiers,
# ignored tests, and cumulative milestone closure.
./dev universe
```

Focused test commands are available when iterating on one surface:

```bash
./dev test ir
./dev test rust-api
./dev test python
./dev test typescript
./dev test php
./dev test ruby

./dev lint --profile rust-clippy-all-features

./dev verify product.integration
./dev verify persistence
./dev verify milestone.m14
```

A new developer machine should be considered fully qualified only after
`./dev universe` passes.

### Copy/paste happy path

After the operating-system packages, Docker, GitHub CLI, and `rustup` are
installed:

```bash
gh auth status
docker version

gh repo clone superkelvint/searchkernel
cd searchkernel

./dev doctor
./dev native-sdk ensure

cargo build -p searchkerneld

./dev check-fast
./dev check
./dev galaxy
./dev universe
```

If any compiler, linker, FlatBuffers, SDK, dependency, cache, or provenance
failure appears during this sequence, run `./dev doctor` first. Do not use
`cargo clean`, delete `target/`, copy SDK directories between machines, or
manually rebuild Vespa as an ad-hoc repair.

## Authority and failure handling

### InstantSearch compatibility prerequisites

`./dev test instantsearch-compat` is the canonical independent integration lane.
It uses the exact dependency lock under `integrations/instantsearch` and runs the
actual pinned InstantSearch.js widgets in Vitest's JSDOM environment. No browser
binary or browser-specific operating-system packages are prerequisites.

The lane runs pin, matrix, documentation, upstream-type, adapter, canonical
binding, request-profile, and real-widget checks through the production adapter
and public endpoint against the dispatcher/IR/Vespa backend. It is owned by PR
CI and Universe. JavaScript dependencies remain ordinary package dependencies;
changing them does not require publishing a browser-capable CI toolchain image.

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
./dev lint --profile rust-clippy-non-native
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

### Remote-only test and verifier execution

An agent that cannot run the repository locally can still execute the canonical
test/verifier surface against an exact pushed commit. If the agent has
authorized GitHub pull-request comment access, post one top-level comment whose
entire body is exactly one of:

```text
/searchkernel-verify ci <exact-40-character-sha>
/searchkernel-verify test <scope> <exact-40-character-sha>
/searchkernel-verify verify <lane> <exact-40-character-sha>
```

Choose `<scope>` and `<lane>` from `build/targets.json`. The resulting
**On-demand verification** run executes the same canonical `./dev` operation
through the trusted default-branch control plane and records exact-SHA evidence.
Do not append prose or additional lines to the command comment, and do not reuse
evidence after the reviewed head changes.

Local-capable agents should continue to prefer `./dev test --remote <scope>`
and `./dev verify --remote <lane>` when the checkout is available. See
`docs/agent-workflow.md` for the remote-only comment/dispatch procedure and
evidence requirements, and `docs/ci.md` for the trust-boundary design.

## Canonical toolchain authorities

Repository tooling must not choose its own interpreter/compiler/runtime. Each
multi-install toolchain has exactly one path authority:

| Toolchain | Authority | Explicit overrides |
| --- | --- | --- |
| Python | `tools/resolve-python.sh` | `PYTHON`, then CI's `CI_PYTHON` |
| Rust | `tools/resolve-rust.sh` | `CARGO`, `RUSTC`, `RUSTFMT` |
| Node.js | `tools/resolve-node.sh` | `NODE`, `NPM` |
| FlatBuffers | `tools/resolve-flatbuffers.sh` | `FLATC_PATH`, `FLATBUFFERS_CPP_ROOT` |
| Native SDK | `tools/resolve-native-sdk.sh` | `SK_NATIVE_SDK`, `SK_NATIVE_TARGET` |

The compatibility wrapper `tools/resolve-flatc.sh` delegates to the canonical
FlatBuffers resolver and must contain no selection logic of its own.

Critical repository scripts must call these authorities instead of invoking bare
`python3`, `cargo`, `rustc`, `rustfmt`, `node`, `npm`, or `flatc`.
For paired tools, the authority also owns pairing: npm is resolved alongside the
selected Node.js installation, and Cargo/Rustc/Rustfmt are exported as one Rust
toolchain view.

`tools/test_toolchain_authorities.py` is the fail-closed architectural guard.
It runs in `./dev check-fast`, `./dev doctor`, and the always-on CI tooling
lane. Reintroducing a direct tool invocation in a protected path is a test
failure, not a convention violation.

Doctor must verify the same downstream paths used by normal checks. It therefore
runs protocol-generation reproducibility, TypeScript generation, FlatBuffers C++
compile/link validation, Cargo metadata, and a real native `searchkerneld`
build. A green doctor means those execution paths are usable, not merely that
similarly named binaries exist somewhere on PATH.

## Shell script syntax

All tracked files with a Bash or POSIX-sh shebang are parsed by
`tools/check-shell-syntax.sh`. The check includes extensionless entrypoints
such as `dev`, and runs in `./dev check-fast`. It uses `bash -n` or `sh -n`
according to the script shebang, so malformed conditionals, unbalanced control
structures, and similar parser errors fail before expensive native verification
begins.

When changing shell tooling, run:

```bash
bash tools/check-shell-syntax.sh
bash tools/test-shell-syntax.sh
```

## Normal development

Run the prerequisite check first when setting up a checkout or diagnosing an
environment failure:

```bash
./dev doctor
```

The normal host workflow uses `target/host` automatically through
`.cargo/config.toml`. Ordinary `./dev check-fast`, standalone Rust API
tests, and non-native workspace checks do not populate the Vespa SDK cache.
Before the first native link, run `./dev native-sdk ensure`. Provisioning
should run this command on every runner/bootstrap cycle: `ensure` checks whether
a newer published native-base generation exists for the same environment, target
and pinned Vespa checkout and atomically refreshes the machine cache when needed.
No manual cache deletion is required. Ordinary Cargo builds use the read-only
`path` lookup and never perform network access or mutate the shared base.

Cargo builds only Search Kernel's `abi.cc` adapter into the target's `OUT_DIR`.
Published generation changes are recorded in a stable receipt inside the shared
base; Cargo watches that receipt and rebuilds only the local adapter after an
automatic base refresh. `cargo clean` is not required.

A fresh or isolated worktree is not expected to already contain a built
`searchkerneld` binary. `./dev doctor` now builds the native daemon through the
canonical Rust/native-SDK authorities, so a successful doctor run proves that
the native compile/link path works. Manual builds remain useful for focused work:

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

### CI and local verification use the immutable image

`docker.io/superkelvint/searchkernel-ci:latest` is the canonical execution
environment for compilation, tests, client verification, and real-Vespa
verification. GitHub Actions and local developer commands use the same pinned
Rust, Node, Ruby, PHP, Python, FlatBuffers, and Vespa base/toolchain.

Image publishing has three independent identities:

- `superkelvint/searchkernel-ci-toolchain:<toolchain-id>` contains the OS,
  compilers, language runtimes, FlatBuffers, and reusable client tooling,
  including the locked TypeScript dependency tree used by generation/tests.
  The toolchain id hashes `docker/searchkernel-ci-toolchain/`,
  `clients/typescript/package.json`, `clients/typescript/package-lock.json`, and
  the resolved repository digest of its upstream base image.
- `superkelvint/searchkernel-ci-vespa:<vespa-commit>-<bundle-id>` contains
  the immutable native runtime closure plus configured Vespa headers/static-link
  metadata. This is the only image that carries the large Vespa/native payload.
- `superkelvint/searchkernel-ci:<searchkernel-revision>` contains only the
  toolchain plus the source-specific validation receipt. During image build,
  the exact Vespa image is mounted read-only with BuildKit for compatibility and
  native-adapter validation; its files are never copied into a committed
  `searchkernel-ci` layer.

Publish only the axis that changed:

```bash
# New client language, compiler/runtime bump, or other CI tooling change.
./dev image toolchain publish

# Pinned Vespa/native bundle change.
./dev image vespa publish

# Normal Search Kernel source publish. This does not republish Vespa bytes.
./dev image publish
```

The leaf records the exact toolchain and Vespa repository digests in labels.
GitHub CI and local `./dev` resolve the Vespa digest from the leaf, materialize
that digest once into a content-addressed Docker volume on the machine, and
mount the volume read-only at `/opt/searchkernel/native-sdk-cache`. Multiple
jobs reuse the same immutable volume; source and Cargo-target volumes remain
job-private. Rust compiler outputs are separately reused through the pinned
runner-local sccache service described below; no target directory is shared.

This means the large Vespa payload is uploaded to Docker Hub only by
`./dev image vespa publish`. An ordinary `./dev image publish` pushes the
toolchain-derived leaf layers and source-validation metadata only. It does not
create or push a composite image and does not copy the Vespa filesystem into
`superkelvint/searchkernel-ci`.

All image publishers require a clean Git checkout. Updating
`searchkernel-ci:latest` additionally requires the local `main` branch.
The literal `latest` and `base-*` leaf tags remain reserved.

The leaf's BuildKit caches are namespaced by the exact toolchain/Vespa
environment identity. Changing either immutable input therefore cannot reuse a
stale native target cache.

These commands dispatch to one disposable image container by default:

```bash
./dev check
./dev check-fast
./dev check-protocol
./dev universe
./dev lint --profile rust-clippy-all-features
./dev test python
./dev test typescript
./dev test php
./dev test ruby
./dev verify integration
./dev verify persistence
```

The local launcher does not bind-mount the developer checkout into the build.
It creates a private Git repository from a bundle of the current `HEAD`, then
overlays exactly the non-ignored tracked/untracked working-tree changes. This
works from ordinary clones, linked Git worktrees, and Docker-outside-of-Docker
developer shells. Ignored state such as `target/`, `node_modules/`, venvs,
local SDKs, and existing build outputs never enters the verification container.
The runner overlays non-ignored tracked and untracked working-tree files so
local verification exercises the source currently being edited.

Each invocation receives a private source volume and a private
`/cargo-target` volume. Those volumes are removed after the command. No
writable Cargo registry, rustup, native SDK, node_modules, or target directory is
shared between concurrent developer/CI workers.

Rust compilation is the deliberate exception to otherwise disposable build
state: the repository pins sccache 0.17.0 with its release checksum. The current
toolchain Dockerfile installs that same binary, while the runner also materializes
it into a versioned read-only Docker volume so rollout does not depend on image
publication order. Once the canonical image contains the pin, runners seed that
volume from the immutable image; older images use the checksum-verified release
artifact only for the bootstrap transition. Docker-backed builds set
`RUSTC_WRAPPER=/opt/searchkernel/sccache/sccache`. One runner-local sccache
server per pinned sccache version and immutable CI toolchain identity owns a
bounded 20 GiB local compiler cache. A toolchain identity change therefore starts
a new compiler-cache namespace instead of reusing the previous environment.
Build containers connect to it over a read-only mounted Unix socket and use
sccache client-side mode, so rustc still executes inside the job's exact-head
container with its private source and `/cargo-target` volumes. The shared cache
is non-authoritative and content-addressed; compiler/cache misses simply compile
normally. CI prints sccache statistics at lane exit. Do not replace this with a
shared Cargo target directory.

Commands that intentionally modify the developer environment or checked-out
source remain host-side: `./dev doctor`, `./dev regen ...`,
`./dev install flatbuffers`, `./dev native-sdk ...`,
and `./dev worktree create ...`.

The image is pulled before local verification by default. Useful overrides are:

```bash
# Deliberately run the legacy host path for debugging/bootstrap work.
SEARCHKERNEL_DEV_USE_CI_IMAGE=0 ./dev check-fast

# Work offline against an already-pulled image.
SEARCHKERNEL_CI_PULL=never ./dev check

# Test another immutable/candidate image.
SEARCHKERNEL_CI_IMAGE=superkelvint/searchkernel-ci:<tag> ./dev universe
```

There is no automatic fallback from Docker to host execution. A missing Docker
daemon/image is an explicit failure so a local green result cannot silently
come from a different toolchain than CI.

## Environment isolation

Container isolation does not by itself make a synthetic test hermetic. Tests that
model resolver, cache, bootstrap, publication, or other environment-sensitive
state MUST start from a clean environment and explicitly declare every variable
they consume. They must not inherit the parent shell and then maintain a
blacklist of known-dangerous `SK_NATIVE_*`, `VESPA_*`, toolchain, or cache
overrides.

Use `tools/run-hermetic-env.sh` for these fixtures. The runner starts with
`env -i`; callers pass execution basics such as `PATH`, a fixture-local
`HOME`/`TMPDIR`, deterministic locale, and the fixture inputs as explicit
`NAME=VALUE` arguments. Ambient variables are unavailable unless the fixture
deliberately opts them in. Regression tests should poison representative parent
variables so accidental ambient authority becomes a deterministic failure.

Supported build/test execution now has two deliberate modes:

| Environment | Cargo target root | Vespa SDK source |
| --- | --- | --- |
| Canonical Docker-backed dev/CI | private `/cargo-target` volume; shared compiler outputs only through sccache | read-only digest-addressed Vespa Docker volume mounted at `/opt/searchkernel/native-sdk-cache` |
| Explicit host override | `target/host` (or caller override) | host resolver/cache or explicit `SK_NATIVE_SDK` |

The Docker-backed path is the default. The host mode exists for toolchain
bootstrap, image construction, low-level environment debugging, and deliberate
custom qualification work; it is not the normal verification contract.

`CARGO_TARGET_DIR` remains the build-output isolation boundary in custom host
runs. Never mount a host `target/`, `~/.cargo`, `~/.rustup`, or mutable
native cache into the canonical image.

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

Python follows the repository-wide toolchain-authority contract above.
`tools/resolve-python.sh` honors `PYTHON` (or CI's selected `CI_PYTHON`),
then a healthy repository `venv/`, and finally a usable system Python >=3.10.
Python generation/check scripts must not invoke a bare `python3`.

Because `./dev doctor` verifies the same protocol-generation path used by
`./dev check`, doctor installs the `codegen` extra when needed. Developers
should normally run `./dev doctor` rather than manually installing these
dependencies.

## Vespa base and adapter builds

Normal development uses the same immutable Vespa Docker bundle as CI. Validate
that canonical bundle explicitly when needed:

```bash
./dev native-sdk ensure
```

This command enters the normal CI-image runner, mounts the exact digest-addressed
`searchkernel-ci-vespa` input referenced by `searchkernel-ci:latest`, and
validates that mounted base. It does not populate a parallel host-native cache.

Host-native SDK provisioning is an explicit compatibility/debugging mode:

```bash
SEARCHKERNEL_DEV_USE_CI_IMAGE=0 ./dev native-sdk ensure
./dev native-sdk path
```

In that explicit host mode, the resolver first checks the machine-global object
for the pinned Vespa checkout hash and the machine-global registration of the one
configured checkout with that same hash. A fresh independent Git clone therefore reuses the installed base and
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

`./dev doctor` validates the selected native SDK and performs a real
`searchkerneld --features native` build. The pinned Vespa source checkout itself
remains optional when a valid explicit or published SDK/toolchain is available;
source-checkout registration is only needed to rebuild or provision a base.

The resolver first checks the machine-global base and registered toolchain
keyed by the checked-in Vespa checkout hash. Normal agent clones therefore need
no clone-local Vespa tree when that machine has already been provisioned.

On a cache miss, `ensure` prefers a compatible already-built local source. For
an already-installed published base, `ensure` also discovers the highest
published release generation matching the current build environment, native
target and pinned Vespa checkout. Release names are generation-bearing:

`searchkernel-vespa-base-v<generation>-<environment>-<target>-<vespa-commit>.tar.gz`

The generation is discovered automatically; callers do not supply or hard-code
it. If the installed published generation is older, `ensure` acquires the
checkout lock and atomically replaces the object/toolchain at the same
checkout-keyed cache path. `./dev native-sdk path` stays strictly offline and
never performs this refresh.

`SK_NATIVE_BASE_ARTIFACT_URL` may point at an explicitly managed mirror. A
canonical generation-bearing filename is sufficient for automatic freshness
comparison; mirrors using another filename may provide
`SK_NATIVE_BASE_ARTIFACT_GENERATION`. Downloads are checksum-verified, their
manifests must exactly match the environment, target and pin, and unsafe archive
entries are rejected before publication.

The published artifact contains the immutable Vespa/runtime base plus the
read-only Vespa header/config/dependency-header toolchain required to compile
Search Kernel's local `abi.cc` adapter. It never contains the Search Kernel
adapter itself.

Native-base release construction and archive qualification are canonical
container operations even for the artifact namespace named `host`. Here
`host` and `ci` identify consumer/cache compatibility namespaces; they do
not authorize execution on the developer host. Release tooling uses:

```bash
./dev native-base build <host|ci> <output-dir> [generation]
./dev native-base verify <archive> <host|ci> <target> <vespa-commit>
```

The launcher snapshots explicitly declared `VESPA_ROOT` and `VESPA_DEPS`
producer inputs into invocation-private Docker volumes, builds and verifies with
the canonical image/toolchain, and exports only the completed artifact bytes.
The low-level builder and archive verifier fail closed when invoked outside that
container path. The GitHub publisher remains host-side only for release
orchestration and byte upload/download; it delegates every build and executable
verification step to these `./dev` commands.

`tools/package-native-base-artifact.sh` creates the bundle, and
`.github/workflows/publish-native-base.yml` invokes
`tools/publish-native-base.sh` from `main`. The publisher allocates the next
generation once and publishes matching host and CI assets under that generation.

For credential-isolated containers, run `tools/fetch-native-base.sh ci` on the
host during every provisioning cycle. It uses the host's GitHub credentials,
reuses a checksum-verified local download when current, and prints the newest
artifact path. Mount that artifact cache read-only and pass the resulting
`file://` path as `SK_NATIVE_BASE_ARTIFACT_URL`; the container never needs a
GitHub token, and `./dev native-sdk ensure` refreshes its shared cache whenever
the host-selected generation advances.

Only when the published artifact is unavailable may an already-configured pinned
Vespa checkout be used as the explicit last-resort bootstrap; if its DSOs are
absent, `ensure` may build the configured
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
./dev lint --profile rust-clippy-all-features
cargo test --workspace --all-targets --all-features
```

Use the no-default-features lane when intentionally verifying Rust/FlatBuffers
without the optional Vespa SDK:

```text
./dev lint --profile rust-clippy-non-native
cargo test --workspace --all-targets --no-default-features
```

There must be zero compiler and Clippy warnings.

The canonical workspace lint profiles are owned by `./dev lint`. In CI, the
runner also writes a repair packet containing the complete rendered diagnostics,
machine-readable Cargo/Clippy messages, the failure classification, and the exact
reproduction command. A `BUILD_OR_COMPILE_FAILURE` packet means no Clippy lint
was observed before the build failed; run `./dev doctor` before changing
product code.

## Native build verification

### Clean-runner real-engine verification

A provisioned runner does not need an `upstream/vespa` source checkout to run
Search Kernel's real-engine verifier targets. The published native-base
toolchain is accepted as pinned Vespa provenance when its environment, target,
release generation, pinned commit, static-link closure, and canonical adapter
compile/link metadata all validate.

Provision the base, then run verifier targets normally:

```bash
./dev native-sdk ensure
./dev verify m14
./dev verify product.rust-real-engine
./dev verify integration
```

On a published-toolchain runner, these targets execute the real SearchKernel /
Vespa engine and compare behavior against the checked-in frozen expected corpus.
They do not rebuild or rerun the upstream Java/C++ oracle implementations.

A qualification machine with the clean configured pinned Vespa source checkout
continues to run live upstream oracles by default. To require qualification mode
explicitly:

```bash
SEARCHKERNEL_VERIFY_LIVE_ORACLE=required ./dev verify m14
```

That fails closed when only a published toolchain is available.
`SEARCHKERNEL_VERIFY_LIVE_ORACLE=skip` deliberately replays the frozen corpus
even on a source qualification host.

The distinction is intentional: qualification proves that frozen expected
behavior still agrees with the pinned upstream oracle; clean-runner verification
proves that the current Search Kernel real-engine path still agrees with that
qualified contract.

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
