# M1 build-system authority inventory

This inventory is the **M1 “inventory before refactoring” evidence** required by
`implementation-plan/build-system-roadmap.md`. It describes the current tree; it
is not a new build contract and does not promote compatibility paths into
architecture.

Snapshot:

- audited tree: `main` at `287fe2413e0af376340d59b39146791fd79e9935`;
- design audit baseline: `19ddb707eb2e966e49daab09d011252bb5bfef1c`;
- tracker: #819; atomic inventory work: #825.

The classifications are the roadmap vocabulary:

- **CANONICAL AUTHORITY** — owns the decision today;
- **CALLER** — consumes a canonical authority without redefining it;
- **TEMPORARY COMPATIBILITY** — working secondary path that later convergence
  is expected to remove or fold into a canonical authority;
- **ORACLE AUTHORING** — intentionally independent reference/golden tooling;
- **DEAD / SUSPECTED DEAD** — no current supported caller was found; prove dead
  before deletion.

## Executive findings

1. `./dev` is the public repository front door, but current `main` does **not**
   yet implement the complete M1 vocabulary. `build`, `ci`, `test --list`,
   `test --remote`, `verify --remote`, and `release-check` are absent.
2. Test targets were hard-coded in `dev`; semantic verifier targets were
   hard-coded separately in `reference/vespa/verify-through.sh`. A separate
   semantic verification policy also existed, but it was not the single public
   test/verifier target registry required by M1. That policy was later retired
   when `build/targets.json` became the canonical registry.
3. The three independently versioned Docker image axes are real and active:
   toolchain, Vespa/native, and the thin Search Kernel leaf. Their tags are
   logical coordinates; immutable execution identity is the resolved repository
   digest. The thin leaf records exact toolchain/Vespa digests and does not
   contain the Vespa payload.
4. Canonical Docker execution fails rather than silently falling back to host
   execution. Host execution exists only behind the explicit
   `SEARCHKERNEL_DEV_USE_CI_IMAGE=0` override.
5. Canonical Docker runs currently create a private writable `/cargo-target`
   volume per invocation and delete it afterward. Shared `sccache` is
   non-authoritative and persistent, but the M1 requirement for persistent
   per-worktree + compatible-environment writable incremental output is not yet
   established or benchmarked. The revised M1 acceptance now explicitly requires
   repeated first-class `./dev test <scope>` runs in fresh canonical containers,
   including a Ruby run that proves no Rust/native rebuild, SDK reconstruction,
   runtime download, or unnecessary code generation on the second run.
6. The compiled-source audit has not drifted: 70 compiled-language files under
   `reference/vespa` = 55 active verifier sources + 14 oracle/golden-authoring
   sources + 1 uncertain compile fixture. The set and role counts were rechecked
   against this snapshot and the design audit baseline. See `compiled-sources.md`.
7. The P0 native-runtime topology/delivery decision remains separate work. Issue
   #807 owns the active static/shared topology investigation; this inventory does
   not change production runtime behavior.
8. Current `main` still registers only `rust-ci.yml` and `rustfmt-autofix.yml`
   under `.github/workflows`. The former `ci-image-runner.yml` now exists at
   `.github/ci/ci-image-runner.yml` as structural/reference data; its header
   explicitly says GitHub Actions does not register it. CI policy/tests and
   verifier routing have moved to that path, but a few consumers still point to
   the removed `.github/workflows/ci-image-runner.yml`, and nine other removed
   workflow names remain referenced by executable/test code. Those stale
   workflow contracts remain convergence debt. Current command outcomes are
   recorded only after re-running them against this snapshot.
9. Oracle execution is not yet explicit-only. `live_oracle_enabled()` defaults
   `SEARCHKERNEL_VERIFY_LIVE_ORACLE` to `auto`, so normal local verifier paths can
   execute live/prebuilt oracles when they are not using the published toolchain.
   This conflicts with the target convergence rule that normal verification
   replays frozen evidence unless a maintainer explicitly opts into live oracle.
10. Package assembly has duplicate behavior. Normal
   `clients/typescript/scripts/verify-package.cjs` proves that a plain npm package
   does **not** bundle `searchkerneld` and requires `SEARCHKERNEL_LOCAL_HOST`, while
   `tools/package-consumer-release.py` temporarily injects the same daemon/native
   closure into both the Python and TypeScript package roots before building the
   release wheel/tarball. `tools/check-consumer-release.py` then verifies that
   injected npm runtime. The support matrix still claims TypeScript/Node only as
   `connect`. These paths must converge before package-manager UX is authoritative.
11. `tools/package-consumer-release.py` directly invokes `cargo`, `pip`, and `npm`
    rather than a future `./dev release-check`/toolchain-resolution authority.
    That is an explicit convergence gap, not a reason to treat the current packager
    as the final release interface.

## Public command and target ownership

| Path / surface | Classification | Current responsibility |
| --- | --- | --- |
| `dev` | **CANONICAL AUTHORITY** | Public repository command router and current test/lint command vocabulary. |
| `BUILDING.md` | **CANONICAL AUTHORITY** | Supported build/toolchain/environment/recovery contract. |
| `build/targets.json` | **CANONICAL AUTHORITY (post-snapshot)** | Current finite test/verifier/change-area registry introduced by build-system convergence. |
| `dev:test_target()` | **TEMPORARY COMPATIBILITY** | Historical hard-coded test scopes before registry convergence. |
| `reference/vespa/verify-through.sh` | **TEMPORARY COMPATIBILITY** | Historical semantic verifier/milestone vocabulary plus legacy aliases consumed by `./dev verify`. |
| `reference/vespa/verifiers/**/verify.sh` | **CALLER** | Semantic lane wrappers behind `verify-through.sh`. |
| `reference/vespa/m*/verify-m*.sh`, `reference/vespa/phase*/verify-*.sh`, `reference/vespa/searchreply/verify-searchreply.sh` | **TEMPORARY COMPATIBILITY** | Historical milestone/current-verifier orchestration retained behind semantic names during convergence. |
| `dev:universe()` | **CALLER** | Cumulative repository qualification; currently invokes canonical `./dev` lanes plus remaining direct tooling. |

Current `./dev --help` exposes `check`, `universe`, `check-fast`,
`check-protocol`, `coverage`, `lint`, `test`, `regen`, `verify`, `install`,
`native-sdk`, `image`, `worktree`, and `doctor`. The M1 design vocabulary that
is not yet present is deliberately recorded as a gap, not represented as if it
already existed.

## Compiler, interpreter, and Cargo authority

| Path / family | Classification | Current responsibility |
| --- | --- | --- |
| `tools/resolve-python.sh` | **CANONICAL AUTHORITY** | Python selection. |
| `tools/resolve-rust.sh` | **CANONICAL AUTHORITY** | Cargo/Rustc/Rustfmt selection as one toolchain view. |
| `tools/resolve-node.sh` | **CANONICAL AUTHORITY** | Node/npm selection. |
| `tools/resolve-flatbuffers.sh` | **CANONICAL AUTHORITY** | `flatc` + FlatBuffers C++ selection. |
| `tools/resolve-native-sdk.sh` | **CANONICAL AUTHORITY** | Native SDK/base selection and host cache lookup. |
| `tools/resolve-flatc.sh` | **TEMPORARY COMPATIBILITY** | Compatibility wrapper; delegates to `resolve-flatbuffers.sh`. |
| `tools/test_toolchain_authorities.py` | **CALLER** | Fail-closed guard against protected scripts bypassing the resolver authorities. |
| `dev` | **CALLER** | Resolves canonical tools once, then invokes Cargo/interpreters for normal repository operations. |
| `.github/workflows/rust-ci.yml` | **TEMPORARY COMPATIBILITY** | PR gate directly embeds `cargo fmt`, `cargo check`, and `cargo clippy`; M2 is intended to delegate this to `./dev ci`. |
| `.github/workflows/rustfmt-autofix.yml` | **CALLER** | Runs pinned rustfmt inside the canonical image for untrusted PR formatting. |
| `.github/ci/ci-image-runner.yml` | **TEMPORARY COMPATIBILITY** | Retained structural/reference workflow contract consumed by CI policy and verifier-routing tests; intentionally outside `.github/workflows`, so GitHub Actions does not register it as a workflow. |
| `reference/vespa/**/rust-driver/Cargo.toml` and `reference/vespa/api-contract/Cargo.toml` | **TEMPORARY COMPATIBILITY** | 18 standalone verifier Cargo workspaces outside the normal product workspace. |
| `tools/check-native-bindings/Cargo.toml`, `tools/rpc-codegen/Cargo.toml` | **CALLER** | Purpose-specific repository tooling Cargo projects. |
| `reference/vespa/**/oracle/build-*.sh` | **ORACLE AUTHORING** | Direct C++/Java oracle compilation for independent behavioral evidence. |

## Docker launchers and three image axes

| Path / image | Classification | Current responsibility |
| --- | --- | --- |
| `docker/searchkernel-ci-toolchain/Dockerfile` | **CANONICAL AUTHORITY** | Reusable OS/compiler/language/FlatBuffers/client-tooling image. |
| `tools/publish-searchkernel-ci-toolchain.sh` | **CANONICAL AUTHORITY** | Computes/publishes `searchkernel-ci-toolchain:<toolchain-id>`. |
| `docker/searchkernel-ci-vespa/Dockerfile` | **CANONICAL AUTHORITY** | Immutable Vespa/native development payload image. |
| `tools/publish-searchkernel-ci-vespa.sh` | **CANONICAL AUTHORITY** | Computes/publishes `searchkernel-ci-vespa:<vespa-commit>-<bundle-id>`. |
| `docker/searchkernel-ci/Dockerfile` | **CANONICAL AUTHORITY** | Thin source-specific leaf; validates exact toolchain/Vespa inputs without copying Vespa payload. |
| `tools/publish-searchkernel-ci-image.sh` | **CANONICAL AUTHORITY** | Publishes `searchkernel-ci:<searchkernel-revision>` and records exact toolchain/Vespa digests. |
| `tools/searchkernel-ci-image-common.sh` | **CANONICAL AUTHORITY** | Shared image identity, digest resolution, clean-checkout, and publication helpers. |
| `tools/run-in-ci-image.sh` | **CANONICAL AUTHORITY** | Default local Docker launcher for Docker-backed check/test/verify commands; fails closed if Docker/image setup is unavailable. |
| `tools/ensure-ci-vespa-volume.sh` | **CANONICAL AUTHORITY** | Materializes the exact leaf-selected Vespa image into digest-addressed read-only runtime storage. |
| `tools/ensure-ci-sccache.sh` | **CALLER** | Provides shared non-authoritative compiler cache keyed by compatible toolchain identity. |
| `.github/workflows/rust-ci.yml` | **TEMPORARY COMPATIBILITY** | Direct Docker launcher for the small PR gate rather than `./dev ci`. |
| `.github/workflows/rustfmt-autofix.yml` | **CALLER** | Pulls the canonical leaf and runs pinned rustfmt inside it before credential-separated writeback. |
| `.github/ci/ci-image-runner.yml` | **TEMPORARY COMPATIBILITY** | Structural/reference runner contract used by CI policy, fixtures, and local verifier routing; it is not a registered Actions workflow. |
| `tools/ci/runner-bootstrap/{install,verify}.sh` | **CALLER** | Self-hosted runner Docker/bootstrap validation. |
| `reference/vespa/{m4,m8,m9,m10}/oracle/run-cpp-oracle.sh` | **ORACLE AUTHORING** | Docker-backed independent oracle execution where needed. |
| `docker/searchkernel-ci/base.Dockerfile` | **DEAD / SUSPECTED DEAD** | Historical composite image recipe that copies Vespa payload into the image. No tracked caller references this actual path; tests only use the basename as a forbidden token or temporary fixture name. Prove dead before deletion. |

The active development/CI Docker **logical coordinates** are:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

The immutable identity used for execution/materialization is the resolved
repository digest, not merely the tag above. This distinction matters because
`publish-searchkernel-ci-vespa.sh` may replace the same logical Vespa tag when
its producer fingerprint changes. The thin-leaf publisher likewise defaults its
tag to `git rev-parse --short=12 HEAD` (and accepts an explicit tag), while the
full Search Kernel commit is recorded separately in the OCI revision label. The
leaf records exact toolchain and Vespa repository digests in OCI labels.
Ordinary Search Kernel leaf publication requires those images to already exist
and does not republish the Vespa bytes.

### Image producer/consumer map

| Logical image | Publisher | Direct consumers |
| --- | --- | --- |
| `searchkernel-ci-toolchain:<toolchain-id>` | `tools/publish-searchkernel-ci-toolchain.sh` | `tools/publish-searchkernel-ci-image.sh` resolves its digest and passes it to `docker/searchkernel-ci/Dockerfile` as the leaf runtime base. |
| `searchkernel-ci-vespa:<vespa-commit>-<bundle-id>` | `tools/publish-searchkernel-ci-vespa.sh` | `tools/publish-searchkernel-ci-image.sh` uses it read-only for leaf build-time compatibility/native validation; `tools/ensure-ci-vespa-volume.sh` materializes the exact leaf-recorded digest for runtime verification. |
| `searchkernel-ci:<searchkernel-revision>` (plus promoted `:latest`) | `tools/publish-searchkernel-ci-image.sh` | `tools/run-in-ci-image.sh`, `.github/workflows/rust-ci.yml`, and `.github/workflows/rustfmt-autofix.yml` pull/run the leaf; the local runner then resolves the leaf-recorded Vespa digest separately. `.github/ci/ci-image-runner.yml` also records this execution contract but is not registered as a workflow. |

This map is intentionally about the **development/CI images**. The host native
base and future consumer-runtime artifact have separate identities and must not
be conflated with these Docker axes.

## Native SDK builders, resolvers, and runtime staging

| Path / family | Classification | Current responsibility |
| --- | --- | --- |
| `core/native/vespa/build-sdk.sh` | **CANONICAL AUTHORITY** | Search Kernel native adapter/SDK assembly from the pinned Vespa build inputs. |
| `tools/resolve-native-sdk.sh` | **CANONICAL AUTHORITY** | Supported native SDK/base resolver and immutable cache authority. |
| `tools/{fetch,install,repair}-native-base*.sh` | **CALLER** | Fetch/install/repair operations around the native-base cache. |
| `tools/{package,publish}-native-base*.sh` | **CANONICAL AUTHORITY** | Immutable native-base packaging/publication mechanics used by the development build input. |
| `tools/prepare-searchkernel-ci-native-context.sh` | **CALLER** | Converts a qualified native SDK/Vespa checkout into the Vespa image build contexts. |
| `core/crates/searchkernel-vespa-bindings/build.rs` | **CALLER** | Cargo/native link integration; consumes the selected base and builds the Search Kernel adapter locally. |
| `reference/vespa/common/verify-common.sh` | **TEMPORARY COMPATIBILITY** | Verifier-specific SDK/runtime preparation layered around the normal native authorities. M3 is intended to collapse this secondary setup. |
| `tools/generate-native-consumer-bundle.py` | **CANONICAL AUTHORITY** | Defines and generates the source/tool bundle required by the packaged native bindings crate. |
| `core/crates/searchkernel-vespa-bindings/consumer_bundle.rs` | **CALLER** | Generated bundle output (`@generated`; do not edit) consumed by the packaged bindings crate. |
| `tools/bundle-native-runtime-closure.py`, `tools/native_runtime_closure.py` | **CANONICAL AUTHORITY** | Computes/stages the relocatable native runtime closure used by current consumer/release tooling. |
| `tools/package-consumer-release.py` | **CANONICAL AUTHORITY** | Assembles the current supported Linux x86-64 consumer artifact set and provenance/checksums. |
| `tools/check-consumer-release.py` | **CALLER** | Verifies the artifact set produced by the consumer release packager. |
| `clients/python/searchkernel/client.py` | **CALLER** | Python managed-local runtime launch path. |
| `clients/typescript/src/node/managed-local-host.ts` | **CALLER** | TypeScript/Node managed-local runtime launch path. |
| `clients/ruby/lib/searchkernel/managed_local.rb` | **CALLER** | Ruby managed-local runtime launch path; product distribution remains deferred by the current support matrix. |
| historical verifier scripts setting `LD_LIBRARY_PATH` or compiling against a verifier SDK | **TEMPORARY COMPATIBILITY** | Secondary verifier runtime/loader setup to be converged after the runtime decision. |
| oracle scripts setting their own loader paths | **ORACLE AUTHORING** | Deliberately independent direct-Vespa reference execution. |

The current production invariant is consumer-safe runtime identity/relocatability,
not any particular DSO layout. The #745 mechanism remains authoritative until a
replacement passes the M1 external-consumer and package-manager proofs.

The current non-oracle verifier/runtime compatibility scripts that still name
`LD_LIBRARY_PATH` are all **TEMPORARY COMPATIBILITY** paths:

```text
reference/vespa/i/verify-i.sh
reference/vespa/m0/verify-m0.sh
reference/vespa/m1/verify-m1.sh
reference/vespa/m10/feature-values/verify-feature-values.sh
reference/vespa/m11/verify-m11-common.sh
reference/vespa/m11/verify-m11-core.sh
reference/vespa/m11/verify-m11-sanitizers.sh
reference/vespa/m13/verify-m13.sh
reference/vespa/m14/verify-m14.sh
reference/vespa/m2/verify-m2.sh
reference/vespa/m3/run-rust.sh
reference/vespa/m5/verify-m5.sh
reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh
reference/vespa/phase9/verify-phase9.sh
reference/vespa/recovery/verify-recovery.sh
reference/vespa/searchreply/verify-searchreply.sh
```

This list is intentionally separate from direct oracle loader setup, which is
**ORACLE AUTHORING**, not a compatibility path to converge into product runtime.

## Standalone verifier workspaces and compiled sources

There are **18 standalone Cargo manifests** under `reference/vespa`:

```text
reference/vespa/api-contract/Cargo.toml
reference/vespa/i/rust-driver/Cargo.toml
reference/vespa/m1/rust-driver/Cargo.toml
reference/vespa/m2/rust-driver/Cargo.toml
reference/vespa/m3/rust-driver/Cargo.toml
reference/vespa/m4/rust-driver/Cargo.toml
reference/vespa/m5/rust-driver/Cargo.toml
reference/vespa/m6/rust-driver/Cargo.toml
reference/vespa/m7/rust-driver/Cargo.toml
reference/vespa/m8/rust-driver/Cargo.toml
reference/vespa/m9/rust-driver/Cargo.toml
reference/vespa/m10/rust-driver/Cargo.toml
reference/vespa/m11/rust-driver/Cargo.toml
reference/vespa/m12/rust-driver/Cargo.toml
reference/vespa/m13/rust-driver/Cargo.toml
reference/vespa/m14/rust-driver/Cargo.toml
reference/vespa/recovery/rust-driver/Cargo.toml
reference/vespa/searchreply/rust-driver/Cargo.toml
```

They are **TEMPORARY COMPATIBILITY** build worlds: active today, but explicitly
part of the later verifier build-graph convergence.

`compiled-sources.md` enumerates every compiled-language source below
`reference/vespa` and classifies it. Current counts are unchanged from the
design audit:

| Role | Rust | C | C++ | Java | Total |
| --- | ---: | ---: | ---: | ---: | ---: |
| Active current-implementation verifier | 49 | 4 | 2 | 0 | **55** |
| Direct oracle/golden authoring | 0 | 0 | 10 | 4 | **14** |
| Uncertain compile fixture | 1 | 0 | 0 | 0 | **1** |
| **Total** | **50** | **4** | **12** | **4** | **70** |

The compiled-language file set is unchanged from the design audit baseline to
this snapshot: diffing `19ddb707...287fe241` under `reference/vespa` yields no
added, removed, or modified `.rs`, `.c`, `.cc`, `.cpp`, or `.java` files. The
current routing still explicitly selects the known API-contract bins and
verifier drivers; `m-api.rs` remains the single source for which no normal
entrypoint was found.

The six active direct native probes are:

| Source | Current runner | Classification |
| --- | --- | --- |
| `reference/vespa/m0/native-probe/probe.c` | `reference/vespa/m0/verify-m0.sh` | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m10/feature-values/native-probe/probe.c` | `reference/vespa/m10/feature-values/verify-feature-values.sh` | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/m9-general-grouping-verifier/native-probe/probe.c` | `reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh` | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/phase9/native-probe/probe.c` | `reference/vespa/phase9/verify-phase9.sh` | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/searchreply/native-probe/searchreply_probe.cpp` | `reference/vespa/searchreply/verify-searchreply.sh` | **TEMPORARY COMPATIBILITY** |
| `reference/vespa/searchreply/native-probe/searchreply_metadata_hardening_probe.cpp` | `reference/vespa/searchreply/verify-searchreply.sh` | **TEMPORARY COMPATIBILITY** |

## Direct oracle compilation and execution

The 14 sources marked **ORACLE AUTHORING** in `compiled-sources.md` are the
independent C++/Java reference implementations. Their build/run entrypoints live
under:

- `reference/vespa/m{4,8,9,10,12,13,14}/oracle/`;
- `reference/vespa/m9-general-grouping-verifier/oracle/`;
- `reference/vespa/searchreply/oracle/`;
- `reference/vespa/m3/{oracle,kstem/oracle}/`;
- `reference/vespa/m6/oracle/`;
- `reference/vespa/m7/oracle/`;
- `reference/vespa/m2/oracle/embedded_crud_spike.cpp`.

Golden-authoring entrypoints such as `freeze-goldens.sh`, `freeze-golden.sh`,
and `m3/kstem/freeze-kstem.sh` are **ORACLE AUTHORING**. Direct oracle **build**
and golden-generation remain explicit authoring operations, but current main does
not yet make oracle **execution** explicit-only: `reference/vespa/common/verify-common.sh`
defaults `SEARCHKERNEL_VERIFY_LIVE_ORACLE` to `auto`, and `live_oracle_enabled()`
returns true when the verifier is not using the published Vespa toolchain.
Consequently ordinary local verifier entrypoints including M3/M4/M6-M10,
M12-M14, recursive grouping, phase9, SearchReply, and feature-values can execute
a prebuilt/direct oracle opportunistically. That automatic execution policy is
**TEMPORARY COMPATIBILITY** and is an explicit M3 convergence gap; the target
contract requires normal verification to replay frozen evidence unless a
maintainer opts into live-oracle execution.

## GitHub Actions and CI-only setup

| Path | Classification | Current responsibility |
| --- | --- | --- |
| `.github/workflows/rust-ci.yml` | **TEMPORARY COMPATIBILITY** | Mandatory PR gate. It directly pulls/runs the canonical image and embeds fmt/check/clippy commands. |
| `.github/workflows/rustfmt-autofix.yml` | **CALLER** | Isolated untrusted formatting plus credential-separated writeback. |
| `.github/ci/ci-image-runner.yml` | **TEMPORARY COMPATIBILITY** | Structural/reference runner contract outside the registered workflow directory; it records exact-head execution for CI policy/verifier-routing checks. |
| `.github/ci/resolve-native-adapter-lib.py` | **CALLER** | Active adapter-location helper used by current verifier, client-E2E, and release-packaging paths. |
| `.github/ci/test_resolve_native_adapter_lib.py` | **CALLER** | Regression coverage for the active adapter resolver. |
| `.github/ci/test_rustfmt_autofix.py` | **CALLER** | Regression coverage for the surviving `rustfmt-autofix.yml` workflow. |
| Deleted-workflow policy/tests listed below | **TEMPORARY COMPATIBILITY** | Residual selector/evidence/release/queue logic that still expects workflows removed from the current two-workflow CI surface; each must be reconciled before reuse. |
| `.github/ci/check-python-workflows.sh` + `.github/ci/select-python.sh` | **DEAD / SUSPECTED DEAD** | No surviving workflow calls them; current references are residual tests/docs. Prove obsolete or reconnect deliberately. |
| `tools/ci/runner-bootstrap/**` | **CALLER** | Self-hosted runner packaging/bootstrap/verification. |
| `tools/test-ci-image-publishing.sh` | **CALLER** | Current image-publication regression test; it does not depend on the structural CI runner file. |

The current automatic PR gate is intentionally small, but it does **not** yet
satisfy the M2 target state where GitHub Actions delegates the gate to
`./dev ci`.

## Release/package builders, support rows, and package-manager artifacts

`docs/release-support.json` is the machine-readable **CANONICAL AUTHORITY** for
current supported/deferred release rows. The supported rows at this snapshot are:

| Surface | Runtime | Evidence | Verification |
| --- | --- | --- | --- |
| `rust-embedded` | `native` | `native-full` | `./dev verify persistence` |
| `searchkerneld` | `native` | `client-e2e` | `tools/check-client-contracts.py --run-e2e` |
| `python` | `managed-local` | `native-full` | `tools/run-python-native-e2e.sh --prevalidated-native-sdk` |
| `python` | `connect` | `client-e2e` | `tools/check-client-contracts.py --run-e2e` |
| `typescript-node` | `connect` | `client-e2e` | `tools/check-client-contracts.py --run-e2e` |

The same file defers these exact identifiers:

```text
typescript-browser-grpc-web
php
ruby
go
macos
windows
linux-aarch64
```

This current support matrix is not evidence that the M1 promised
Cargo/pip/npm/RubyGems lazy-runtime feasibility proof is complete.

A current implementation mismatch is deliberately recorded here. Normal
`clients/typescript/scripts/verify-package.cjs` creates a clean npm consumer and
asserts that the installed package cannot resolve a bundled managed-local daemon;
it must instruct the user to set `SEARCHKERNEL_LOCAL_HOST`. By contrast,
`tools/package-consumer-release.py` temporarily copies `bin/` + `lib/` runtime
trees into both `clients/python/searchkernel` and `clients/typescript` before
building the release wheel/npm tarball, and `tools/check-consumer-release.py`
requires a runnable daemon tree in that npm artifact. The support matrix still
exposes only `typescript-node` / `connect`. Treat the support matrix as the
support authority and these divergent assembly paths as convergence evidence;
do not infer managed-local TypeScript release support from the release artifact
contents alone.

Package-manager surfaces present in the repository are:

| Artifact | Classification | Notes |
| --- | --- | --- |
| `core/crates/searchkernel/Cargo.toml` (+ workspace `Cargo.toml`) | **CANONICAL AUTHORITY** | Rust package/product surface. |
| `clients/python/pyproject.toml` | **CANONICAL AUTHORITY** | Python package metadata. |
| `clients/typescript/package.json` | **CANONICAL AUTHORITY** | npm package metadata. |
| `clients/ruby/searchkernel.gemspec` | **CANONICAL AUTHORITY** | RubyGems package metadata; release support currently deferred. |
| `clients/php/composer.json` | **CANONICAL AUTHORITY** | Composer client metadata; not one of the four M1 runtime-feasibility surfaces. |

Current release/native artifact builders and checkers include:

- `tools/package-consumer-release.py` — **CANONICAL AUTHORITY** for assembling
  the current supported Linux x86-64 consumer artifact set and its provenance;
- `tools/check-consumer-release.py` — **CALLER** that verifies that artifact set;
- `tools/generate-native-consumer-bundle.py` — **CANONICAL AUTHORITY** for the
  checked-in Rust consumer bundle mapping;
- `tools/package-native-base-artifact.sh` and `tools/publish-native-base.sh` —
  **CANONICAL AUTHORITY** for the development native-base artifact, distinct
  from the future consumer-runtime artifact;
- `tools/check-release-support.py` and
  `tools/check-release-persistence-contract.py` — **CALLER** release policy
  checks.

## External consumer and package tests

| Path | Classification | What it exercises |
| --- | --- | --- |
| `tools/test-native-consumer-package.sh` | **CALLER** | Clean external Rust/native consumer package behavior. |
| `tools/test_package_consumer_release.py` | **CALLER** | Consumer release packager/verifier behavior. |
| `clients/python/tests/test_package_import.py` | **CALLER** | Source-tree public-export/import regression only; it does **not** prove external wheel installation. |
| `tools/check-consumer-release.py` (Python wheel path) | **CALLER** | Installs the built wheel into a temporary target and verifies the packaged daemon/native runtime with repository/runtime overrides removed. |
| `clients/python/tests/test_managed_local_host.py` | **CALLER** | Python managed-local runtime hosting behavior. |
| `clients/typescript/scripts/verify-package.cjs` | **CALLER** | npm package contents/install behavior. |
| `clients/typescript/scripts/verify-installed-managed-local-e2e.cjs` | **CALLER** | Installed TypeScript managed-local E2E path. |
| `clients/typescript/tests/managed-local-host.test.ts` | **CALLER** | Node managed-local host behavior. |
| `clients/php/tests/verify_package.sh` + `package_consumer.php` | **CALLER** | Composer/package consumer behavior. |
| `dev:run_ruby_tests()` + `clients/ruby/searchkernel.gemspec` | **CALLER** | Builds and installs the gem into an isolated `GEM_HOME`, validates the installed package, then runs `try_searchkernel_client_api_e2e.rb` and `try_searchkernel_package_daemon_e2e.rb` from that installed gem. |
| `clients/ruby/examples/run_searchkernel_package_daemon_e2e.sh` | **CALLER** | Builds/installs the Ruby gem in an isolated `GEM_HOME`, clears repository load-path overrides, and runs the real HTTP client against an externally owned daemon. |
| `tools/run-python-native-e2e.sh --include-facade` | **CALLER** | Runs Python gRPC E2E and `clients/python/examples/try_searchkernel_client_api_e2e.py` under the shared managed-local runtime; the client-contract catalog invokes this combined lane. |
| `reference/vespa/common/test-published-provenance.sh` | **CALLER** | Published native provenance contract. |

These tests prove pieces of current packaging behavior; they do not by
themselves prove the M1 common lazy-cache mechanism across Cargo, pip, npm, and
RubyGems.

## Ambiguous or suspected-dead paths

These items should not be silently carried forward:

1. `reference/vespa/api-contract/src/bin/m-api.rs` — **DEAD / SUSPECTED DEAD**.
   It is described as a compile-only stable API fixture, but the design audit and
   this inventory found no normal milestone/product verifier entrypoint for it.
   M3 must prove active then migrate it, or prove obsolete then delete it.
2. `docker/searchkernel-ci/base.Dockerfile` — **DEAD / SUSPECTED DEAD** as a
   production image path. It represents the older composite model that copies
   the Vespa payload. Current publishers use the thin
   `docker/searchkernel-ci/Dockerfile`; `git grep` finds no caller of the actual
   path. Regression/identity tests mention only the basename as a forbidden token
   or temporary fixture name.
3. Deleted workflow dependencies — **TEMPORARY COMPATIBILITY** until
   reconciled. The current workflow directory contains only `rust-ci.yml` and
   `rustfmt-autofix.yml`, while executable policy/test code still references the
   deleted workflow surface below. `unit-tests.yml` is intentionally different:
   `check-ci-evidence-ownership.py` names it only as a file that must stay retired.

Confirmed missing workflow names still consumed as if present (the list excludes
`ci-image-runner.yml`, whose structural/reference file now lives under
`.github/ci/`):

| Missing workflow | Tracked executable/test dependents |
| --- | --- |
| `abi-surface.yml` | `.github/ci/test_native_verifier_runner.py`; `tools/test-native-build-hardening.sh`; `tools/test-native-environment-fingerprint.sh` |
| `behavioral-verification.yml` | `.github/ci/behavioral_lanes_required.py`; `.github/ci/check-ci-evidence-ownership.py`; `.github/ci/native_verifier_required.py`; `.github/ci/test_behavioral_lanes_required.py`; `.github/ci/test_coverage_verifier_required.py`; `.github/ci/test_native_verifier_required.py`; `.github/ci/test_tooling_invariant_required.py` |
| `client-capabilities.yml` | `tools/test_package_consumer_release.py` |
| `issue-queue-reconcile.yml` | `tools/test_check_issue_queue.py` |
| `native-verifier-runner.yml` | `.github/ci/native_verifier_required.py`; `.github/ci/test_native_verifier_required.py` |
| `nightly.yml` | `.github/ci/coverage_verifier_required.py`; `.github/ci/test_coverage_verifier_required.py` |
| `persistence-compatibility.yml` | `.github/ci/test_ci_image_runner_isolation.py`; `tools/check-release-persistence-contract.py` |

The remaining references to `ci-image-runner.yml` are split between the
current structural/reference file and stale workflow-path contracts. The
following four tracked files still dereference the removed
`.github/workflows/ci-image-runner.yml` path:

```text
tools/test-native-environment-fingerprint.sh
tools/check-native-consumer-contract.py
tools/check-release-persistence-contract.py
tools/check-release-support.py
```

The current structural/reference consumers use `.github/ci/ci-image-runner.yml`:

```bash
git grep -l 'ci-image-runner.yml' -- .github tools reference | sort
```

This broad search returns both valid structural-file consumers and the stale
workflow-path contracts above; inspect the resolved path before treating a name
match as a missing workflow.

Reproduce the broad path-name inventory with:

```bash
git grep -l ci-image-runner.yml -- .github tools reference | sort
```

Current observable consequences re-run from this snapshot:

- `./dev check-fast` passes, including verifier routing through the structural
  runner at `.github/ci/ci-image-runner.yml`;
- `python3 tools/test_package_consumer_release.py -v` errors in
  `test_client_capability_workflow_watches_every_rust_bundle_input` because
  `.github/workflows/client-capabilities.yml` is absent;
- `python3 tools/test_check_issue_queue.py -v` errors in
  `test_write_capable_reconciler_uses_trusted_pr_close_trigger` because
  `.github/workflows/issue-queue-reconcile.yml` is absent.

These errors are useful inventory evidence, not a reason to restore historical
workflow topology blindly. M2 must decide which policy survives behind the new
thin `./dev ci` / on-demand verification model and delete obsolete guards only
after their intended invariant is either migrated or explicitly retired.

## Reproducing the compiled-source audit

Run from the repository root:

```bash
# All compiled-language sources in the reference/Vespa tree.
git ls-files 'reference/vespa/**' \
  | grep -E '\.(rs|c|cc|cpp|java)$' \
  | sort

# The checked-in Markdown inventory must enumerate that exact set and totals.
python3 - <<'PY'
from collections import Counter
from pathlib import Path
import re, subprocess

text = Path('implementation-plan/audit/build-system/compiled-sources.md').read_text()
rows = []
for line in text.splitlines():
    match = re.match(r'^\| `([^`]+)` \| ([^|]+) \| ([^|]+) \| \*\*([^*]+)\*\* \|$', line)
    if match:
        rows.append(tuple(part.strip() for part in match.groups()))
tracked = subprocess.check_output([
    'git', 'ls-files', 'reference/vespa/**'
], text=True).splitlines()
compiled = sorted(p for p in tracked if re.search(r'\.(rs|c|cc|cpp|java)$', p))
assert sorted(row[0] for row in rows) == compiled
print(Counter(row[1] for row in rows))
print(Counter(row[2] for row in rows))
PY
```

The set assertion must succeed. The role counts must be 55 active, 14 oracle,
and 1 uncertain; language totals must be 50 Rust, 4 C, 12 C++, and 4 Java.

## M1 inventory coverage matrix

| Required M1 inventory category | Concrete evidence in this inventory |
| --- | --- |
| Public `./dev` commands | “Public command and target ownership” records `dev`, its current command surface, and missing M1 commands. |
| Direct Cargo/compiler/interpreter invocations | “Compiler, interpreter, and Cargo authority” records canonical resolvers, `dev`, direct PR-CI Cargo, standalone verifier Cargo, and oracle compiler paths; release-packager direct tool use is also called out explicitly. |
| Docker launchers | “Docker launchers and three image axes” records the local runner, both surviving workflows that invoke the leaf, publishers, runner bootstrap, and Docker-backed oracle execution. |
| Image publishers/consumers | “Image producer/consumer map” names the publisher and direct consumers for all three image axes. |
| Native SDK builders/resolvers | “Native SDK builders, resolvers, and runtime staging” records `build-sdk.sh`, `resolve-native-sdk.sh`, native-base cache operations, and CI-native context preparation. |
| Runtime staging and loader behavior | The native/runtime table records consumer-bundle generation, runtime-closure staging, release packaging, managed-local hosts, verifier loader compatibility, and oracle loader isolation. |
| Standalone verifier workspaces | The exact 18 `reference/vespa/**/Cargo.toml` paths are listed. |
| Active C/C++ verifier compilation | The exact four C and two C++ current-implementation probe sources and their runners are listed. |
| Direct oracle compilation/execution | `compiled-sources.md` marks all 14 oracle sources; “Direct oracle compilation and execution” records their build/run and golden-authoring entrypoints. |
| GitHub Actions workflows | “GitHub Actions and CI-only setup” records both registered workflows, the non-registered structural runner file, nine missing workflow dependencies still referenced by executable/test code, and the intentionally retired `unit-tests.yml` sentinel. |
| Release/package builders | The release/native section records the consumer release packager/checker, native-base packager/publisher, and generated Rust consumer-bundle authority. |
| Support-matrix rows | The five supported rows and seven exact deferred identifiers from `docs/release-support.json` are recorded. |
| Package-manager artifacts | Cargo, `pyproject.toml`, npm `package.json`, RubyGems `.gemspec`, and Composer metadata are listed and classified. |
| External-consumer/package tests | The external-consumer table distinguishes source-only Python import coverage from real wheel install/runtime proof, records Python public-facade E2E, and covers Rust, npm/Node, Composer/PHP, Ruby installed-gem managed-local and external-daemon E2E, and published-provenance checks. |

## M1 handoff from this inventory

This inventory establishes the “what exists today” baseline only. The next M1
implementation work must not infer completion from the existence of these
paths. In particular, the following remain explicit acceptance gaps:

- create the single GitHub-readable test/verifier target registry and complete
  public command vocabulary;
- preserve the image axes while testing fail-closed identity behavior;
- implement/confirm persistent per-worktree compatible writable build state and
  collect the required warm-performance baseline, including repeated language
  scope tests in freshly created canonical containers and the explicit Ruby
  no-rebuild acceptance;
- finish #807 and the P0 runtime ADR/external-consumer proof;
- prove the same chosen runtime delivery model behind minimal Cargo, pip, npm,
  and RubyGems feasibility prototypes.
