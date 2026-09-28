# M1 build-system authority inventory

This inventory is the **M1 “inventory before refactoring” evidence** required by
`implementation-plan/build-system-roadmap.md`. It describes the current tree; it
is not a new build contract and does not promote compatibility paths into
architecture.

Snapshot:

- audited tree: `main` at `128291d92ed219ce74a2df4890bf59c66b714694`;
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
2. Test targets are hard-coded in `dev`; semantic verifier targets are hard-coded
   separately in `reference/vespa/verify-through.sh`. `verification/dag.toml`
   owns semantic verification relationships, but it is not the single public
   test/verifier target registry required by M1.
3. The three independently versioned Docker image axes are real and active:
   toolchain, Vespa/native, and the thin Search Kernel leaf. The thin leaf records
   exact immutable image identities and does not contain the Vespa payload.
4. Canonical Docker execution fails rather than silently falling back to host
   execution. Host execution exists only behind the explicit
   `SEARCHKERNEL_DEV_USE_CI_IMAGE=0` override.
5. Canonical Docker runs currently create a private writable `/cargo-target`
   volume per invocation and delete it afterward. Shared `sccache` is
   non-authoritative and persistent, but the M1 requirement for persistent
   per-worktree + compatible-environment writable incremental output is not yet
   established or benchmarked.
6. The compiled-source audit has not drifted: 70 compiled-language files under
   `reference/vespa` = 55 active verifier sources + 14 oracle/golden-authoring
   sources + 1 uncertain compile fixture. See `compiled-sources.md`.
7. The P0 native-runtime topology/delivery decision remains separate work. Issue
   #807 owns the active static/shared topology investigation; this inventory does
   not change production runtime behavior.
8. The reduced workflow set left stale CI-policy/test callers behind. Seventeen pre-existing
   tracked helpers still reference the deleted `.github/workflows/ci-image-runner.yml`;
   `tools/test-ci-image-publishing.sh` is still called by `./dev check-fast`, so
   current `main` fails that fast gate before product compilation. This is a
   pre-existing convergence defect, not evidence from this documentation change.

## Public command and target ownership

| Path / surface | Classification | Current responsibility |
| --- | --- | --- |
| `dev` | **CANONICAL AUTHORITY** | Public repository command router and current test/lint command vocabulary. |
| `BUILDING.md` | **CANONICAL AUTHORITY** | Supported build/toolchain/environment/recovery contract. |
| `dev:test_target()` | **TEMPORARY COMPATIBILITY** | Hard-coded test scopes: `ir`, `rust-api`, `python`, `typescript`, `php`, `ruby`; no `test --list` yet. |
| `reference/vespa/verify-through.sh` | **TEMPORARY COMPATIBILITY** | Hard-coded semantic verifier/milestone vocabulary plus legacy aliases consumed by `./dev verify`. |
| `reference/vespa/verifiers/**/verify.sh` | **CALLER** | Semantic lane wrappers behind `verify-through.sh`. |
| `verification/dag.toml` | **CANONICAL AUTHORITY** | Semantic verification ownership/dependency graph. It does not currently replace the public target lists above. |
| `verification/dag.py` and `verification/test_dag.py` | **CALLER** | Load, validate, and project the semantic verification DAG where invoked. |
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
| `tools/run-in-ci-image.sh` | **CANONICAL AUTHORITY** | Default local Docker launcher for build/test/verify commands; fails closed if Docker/image setup is unavailable. |
| `tools/ensure-ci-vespa-volume.sh` | **CANONICAL AUTHORITY** | Materializes the exact leaf-selected Vespa image into digest-addressed read-only runtime storage. |
| `tools/ensure-ci-sccache.sh` | **CALLER** | Provides shared non-authoritative compiler cache keyed by compatible toolchain identity. |
| `.github/workflows/rust-ci.yml` | **TEMPORARY COMPATIBILITY** | Direct Docker launcher for the small PR gate rather than `./dev ci`. |
| `tools/ci/runner-bootstrap/{install,verify}.sh` | **CALLER** | Self-hosted runner Docker/bootstrap validation. |
| `reference/vespa/{m4,m8,m9,m10}/oracle/run-cpp-oracle.sh` | **ORACLE AUTHORING** | Docker-backed independent oracle execution where needed. |
| `docker/searchkernel-ci/base.Dockerfile` | **DEAD / SUSPECTED DEAD** | Historical composite image recipe that copies Vespa payload into the image. No supported publisher/runner consumes it; current tests still mention it. Prove dead before deletion. |

The active immutable identities are therefore exactly:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

The leaf additionally records exact repository digests in OCI labels. Ordinary
Search Kernel leaf publication requires the toolchain and Vespa images to
already exist and does not republish the Vespa bytes.

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
| `core/crates/searchkernel-vespa-bindings/consumer_bundle.rs` + `tools/generate-native-consumer-bundle.py` | **CANONICAL AUTHORITY** | Current consumer-safe #745 Rust bundle/runtime metadata path; retained until the M1 runtime replacement is proven. |
| `tools/bundle-native-runtime-closure.py`, `tools/native_runtime_closure.py` | **CANONICAL AUTHORITY** | Computes/stages the relocatable native runtime closure used by current consumer/release tooling. |
| `tools/package-consumer-release.py`, `tools/check-consumer-release.py` | **CALLER** | Assemble and verify current external-consumer release artifacts. |
| `clients/python/searchkernel/client.py` | **CALLER** | Python managed-local runtime launch path. |
| `clients/typescript/src/node/managed-local-host.ts` | **CALLER** | TypeScript/Node managed-local runtime launch path. |
| `clients/ruby/lib/searchkernel/managed_local.rb` | **CALLER** | Ruby managed-local runtime launch path; product distribution remains deferred by the current support matrix. |
| historical verifier scripts setting `LD_LIBRARY_PATH` or compiling against a verifier SDK | **TEMPORARY COMPATIBILITY** | Secondary verifier runtime/loader setup to be converged after the runtime decision. |
| oracle scripts setting their own loader paths | **ORACLE AUTHORING** | Deliberately independent direct-Vespa reference execution. |

The current production invariant is consumer-safe runtime identity/relocatability,
not any particular DSO layout. The #745 mechanism remains authoritative until a
replacement passes the M1 external-consumer and package-manager proofs.

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
and `m3/kstem/freeze-kstem.sh` are also **ORACLE AUTHORING**. They must remain
outside ordinary build/test requirements unless explicitly invoked by a
maintainer.

## GitHub Actions and CI-only setup

| Path | Classification | Current responsibility |
| --- | --- | --- |
| `.github/workflows/rust-ci.yml` | **TEMPORARY COMPATIBILITY** | Mandatory PR gate. It directly pulls/runs the canonical image and embeds fmt/check/clippy commands. |
| `.github/workflows/rustfmt-autofix.yml` | **CALLER** | Isolated untrusted formatting plus credential-separated writeback. |
| `.github/ci/*.py`, `.github/ci/*.sh` | **TEMPORARY COMPATIBILITY** | Exact-head, runner, evidence/ownership, and policy helper/test surface. Several still name deleted reusable workflows and must be reconciled with the reduced CI model. |
| `tools/ci/runner-bootstrap/**` | **CALLER** | Self-hosted runner packaging/bootstrap/verification. |
| `tools/test-ci-image-publishing.sh` | **TEMPORARY COMPATIBILITY** | Active `check-fast` regression test, but currently attempts to read the deleted `ci-image-runner.yml` and fails. |
| `.github/ci/{check-ci-evidence-ownership.py,check-self-hosted-rust-workflows.py,coverage_verifier_required.py,native_verifier_required.py,test_*.py}` and related release/native guards | **TEMPORARY COMPATIBILITY** | Residual policy/tests with references to deleted reusable workflow names; inventory before deciding which logic migrates versus dies. |

The current automatic PR gate is intentionally small, but it does **not** yet
satisfy the M2 target state where GitHub Actions delegates the gate to
`./dev ci`.

## Release/package builders, support rows, and package-manager artifacts

`docs/release-support.json` is the machine-readable **CANONICAL AUTHORITY** for
current supported/deferred release rows. At this snapshot it supports:

- Rust embedded native;
- `searchkerneld` native;
- Python managed-local native;
- Python connect;
- TypeScript/Node connect.

It defers TypeScript browser gRPC-Web, PHP, Ruby, Go, macOS, Windows, and Linux
AArch64. This current support matrix is not evidence that the M1 promised
Cargo/pip/npm/RubyGems lazy-runtime feasibility proof is complete.

Package-manager surfaces present in the repository are:

| Artifact | Classification | Notes |
| --- | --- | --- |
| `core/crates/searchkernel/Cargo.toml` (+ workspace `Cargo.toml`) | **CANONICAL AUTHORITY** | Rust package/product surface. |
| `clients/python/pyproject.toml` | **CANONICAL AUTHORITY** | Python package metadata. |
| `clients/typescript/package.json` | **CANONICAL AUTHORITY** | npm package metadata. |
| `clients/ruby/searchkernel.gemspec` | **CANONICAL AUTHORITY** | RubyGems package metadata; release support currently deferred. |
| `clients/php/composer.json` | **CANONICAL AUTHORITY** | Composer client metadata; not one of the four M1 runtime-feasibility surfaces. |

Current release/native artifact builders and checkers include:

- `tools/package-consumer-release.py` — **CALLER**;
- `tools/check-consumer-release.py` — **CALLER**;
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
| `clients/python/tests/test_package_import.py` | **CALLER** | Python package import/install shape. |
| `clients/python/tests/test_managed_local_host.py` | **CALLER** | Python managed-local runtime hosting behavior. |
| `clients/typescript/scripts/verify-package.cjs` | **CALLER** | npm package contents/install behavior. |
| `clients/typescript/scripts/verify-installed-managed-local-e2e.cjs` | **CALLER** | Installed TypeScript managed-local E2E path. |
| `clients/typescript/tests/managed-local-host.test.ts` | **CALLER** | Node managed-local host behavior. |
| `clients/php/tests/verify_package.sh` + `package_consumer.php` | **CALLER** | Composer/package consumer behavior. |
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
   `docker/searchkernel-ci/Dockerfile`; only regression/identity tests were found
   referencing `base.Dockerfile`.
3. Deleted reusable-workflow references — **TEMPORARY COMPATIBILITY** until
   reconciled. The current workflow directory contains only `rust-ci.yml` and
   `rustfmt-autofix.yml`, while 17 tracked helpers still reference
   `ci-image-runner.yml`. Some encode still-useful policy and some may now be
   dead. They must not be treated as active CI authorities merely because their
   files remain tracked.

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

## M1 handoff from this inventory

This inventory establishes the “what exists today” baseline only. The next M1
implementation work must not infer completion from the existence of these
paths. In particular, the following remain explicit acceptance gaps:

- create the single GitHub-readable test/verifier target registry and complete
  public command vocabulary;
- preserve the image axes while testing fail-closed identity behavior;
- implement/confirm persistent per-worktree compatible writable build state and
  collect the required warm-performance baseline;
- finish #807 and the P0 runtime ADR/external-consumer proof;
- prove the same chosen runtime delivery model behind minimal Cargo, pip, npm,
  and RubyGems feasibility prototypes.
