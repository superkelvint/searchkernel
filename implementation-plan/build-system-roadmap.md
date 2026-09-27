# Search Kernel Build-System Convergence Roadmap

Status: proposed implementation roadmap  
Design authority: `implementation-plan/build-system.md`  
Scope: implementation sequencing only; this document does not redefine the architecture/PRD

## 1. Purpose

`build-system.md` defines the desired build-system architecture, personas, distribution model, and Definition of Done.

This roadmap answers:

> **In what order do we change the repository so the desired architecture becomes true without breaking existing developer, agent, verifier, or consumer workflows?**

This roadmap deliberately has only **six milestones**. The detailed work inside each milestone should be decomposed into small issues/PRs; those sub-items are not separate milestones.

The migration rules are:

1. preserve working behavior before cleanup;
2. create the canonical authority before deleting duplicate implementations;
3. keep automatic PR CI small;
4. keep heavy verification explicitly available, including to remote-only coding agents;
5. preserve the existing three-axis Docker image architecture;
6. never rebuild or republish Vespa for ordinary Search Kernel source changes;
7. make Cargo/npm/pip/RubyGems the end-user installation surfaces;
8. tie review/release evidence to exact commits and actual distributable artifacts;
9. keep oracle authoring separate from normal verification;
10. delete compatibility paths only after replacement proof.

---

## 2. Target end state

```text
                         DEVELOPMENT / VERIFICATION

developer / local-capable agent                 remote-only coding agent
             |                                           |
             v                                           v
           ./dev                              GitHub on-demand workflow
             |                                           |
             +-------------------+-----------------------+
                                 |
                                 v
                     canonical Search Kernel environment
                                 |
                    +------------+------------+
                    |                         |
                    v                         v
                 Cargo                  native build
                    |                         |
                    +------------+------------+
                                 |
                                 v
                    canonical test/verifier semantics


                      DEVELOPMENT IMAGE DISTRIBUTION

searchkernel-ci-toolchain:<toolchain-id>
                    |
                    +-------------------+
                                        |
searchkernel-ci-vespa:<vespa>-<bundle>  |
                    |                   |
                    +---------+---------+
                              |
                              v
              thin searchkernel-ci:<revision>
                 records exact immutable inputs
                 does not copy Vespa payload


                         PRODUCT DISTRIBUTION

qualified common managed-local runtime payload
              |
              +--> standalone searchkerneld archive
              +--> Python package
              +--> TypeScript/Node package
              +--> Ruby package

Rust
              |
              v
normal Cargo package mechanics transparently
provide/link/provision the chosen native runtime

User installation surfaces:

    cargo add searchkernel
    pip install searchkernel
    npm install @searchkernel/client
    gem install searchkernel
```

---

## 3. Milestone overview

```text
M1  Canonical build foundation
    commands + target registry + image authorities + cache/isolation
                      |
                      v
M2  Verification access
    thin PR CI + exact-SHA remote-only GitHub verification
                      |
                      v
M3  Verifier convergence
    normal build graph + shared runtime + explicit oracle + simple Universe
                      |
                      v
M4  Product distribution
    native runtime decision + Cargo/npm/pip/RubyGems managed-local packaging
                      |
                      v
M5  Release and developer UX
    release-check + support matrix + bootstrap/docs
                      |
                      v
M6  Cleanup and final qualification
    remove duplicate authorities + persona/adversarial acceptance
```

M1 is the foundation. Parts of M2 and M3 can proceed in parallel after their M1 prerequisites exist. M4 must not finalize until the native runtime distribution decision is proven. M5 consumes the outputs of M2–M4. M6 is deliberately last.

---

# Milestones

## M1 — Canonical build foundation

### Goal

Make the build system's authorities explicit before migrating behavior.

This milestone establishes the foundation used by every later milestone:

- one public command vocabulary;
- one target registry;
- one canonical development environment;
- the preserved three-axis Docker image model;
- clear mutable/immutable cache ownership.

### 1. Inventory before refactoring

Create a checked-in inventory under:

```text
implementation-plan/audit/build-system/
```

covering:

- public `./dev` commands;
- direct Cargo/compiler/interpreter invocations;
- Docker launchers;
- image publishers and consumers;
- native SDK builders/resolvers;
- runtime staging and loader behavior;
- standalone verifier workspaces;
- active C/C++ verifier compilation;
- direct oracle compilation/execution;
- GitHub Actions workflows;
- release/package builders;
- support-matrix rows;
- external-consumer/package tests.

Classify each item as:

```text
CANONICAL AUTHORITY
CALLER
TEMPORARY COMPATIBILITY
ORACLE AUTHORING
DEAD / SUSPECTED DEAD
```

Explicitly record the existing image architecture:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

and the verifier baseline:

```text
55 active current-implementation compiled sources
14 direct oracle/golden-authoring sources
1 uncertain compile fixture
```

### 2. Canonical command/target registry

Introduce one repository-owned authority for public test/verifier targets.

It must drive:

```bash
./dev test --list
./dev verify --list
./dev test <scope>
./dev verify <lane>
```

and later remote-workflow target validation.

Add/normalize:

```bash
./dev build
./dev ci
./dev test --list
./dev verify --list
```

Definitions:

- `build` = canonical supported product build in the canonical environment;
- `ci` = intentionally cheap mandatory PR gate;
- `test --list` / `verify --list` = finite supported vocabulary.

The registry must be GitHub-readable so remote-only agents can discover targets without running `./dev`.

### 3. Preserve the three-axis Docker model

Make these explicit authorities, not cleanup candidates:

#### Toolchain image

```text
searchkernel-ci-toolchain:<toolchain-id>
```

Owns compilers, language runtimes, FlatBuffers, reusable client/build tooling, and pinned sccache tooling.

#### Vespa/native image

```text
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
```

Owns the large immutable Vespa/native build input, headers, metadata, and provenance.

#### Thin Search Kernel leaf

```text
searchkernel-ci:<searchkernel-revision>
```

Records exact toolchain and Vespa identities, validates compatibility, and does **not** copy Vespa into each source-specific leaf.

Preserve:

```bash
./dev image toolchain publish
./dev image vespa publish
./dev image publish
```

Invariant:

> Ordinary Search Kernel source changes must never rebuild or republish the large Vespa payload.

### 4. Cache and worktree ownership

Implement/confirm:

- immutable toolchain/Vespa inputs shared by immutable identity;
- Vespa materialized read-only from digest-addressed storage;
- sccache shared only as a non-authoritative content cache;
- writable Cargo/native incremental state isolated per worktree + compatible environment identity;
- no writable target tree shared across unrelated worktrees/agents;
- CI writable state job-private.

If measurement shows private target dirs + sccache outperform or equal persistent per-worktree targets, document the evidence before changing the PRD expectation.

### M1 acceptance

- a reviewer can identify every build/runtime authority without archaeology;
- `./dev build`, `./dev ci`, `./dev test --list`, and `./dev verify --list` exist;
- local and future remote callers consume one target registry;
- image axes remain independently versioned/published;
- an ordinary source publish does not copy/re-upload Vespa;
- local worktrees cannot mutate the same writable Cargo target state;
- no existing working path has been deleted merely because it looks redundant.

---

## M2 — Thin CI and first-class remote verification

### Goal

Give both local-capable and remote-only agents reliable verification without restoring the old heavy mandatory PR matrix.

### 1. Automatic PR CI delegates to `./dev ci`

The PR gate remains intentionally small:

```text
cargo fmt --check
cargo check
cargo clippy -D warnings
```

with exact semantics owned by `./dev ci`.

GitHub Actions should conceptually become:

```text
checkout exact PR revision
./dev ci
```

Workflow YAML must not independently own Cargo flags, compiler selection, verifier semantics, or native SDK setup.

Clean up contract tests that assert obsolete workflow filenames/internal YAML structure; test the public `./dev ci` contract instead.

### 2. Add one generic on-demand verification workflow

This is a **first-class public interface** for coding agents that have GitHub access but no usable Search Kernel development environment.

Inputs:

```text
revision   exact pushed commit SHA
operation  ci | test | verify
target     known scope/lane where applicable
```

The workflow:

1. validates caller/trust policy;
2. validates operation/target against the M1 registry;
3. checks out the exact requested SHA;
4. proves `git rev-parse HEAD == requested SHA`;
5. runs:
   - `./dev ci`, or
   - `./dev test <scope>`, or
   - `./dev verify <lane>`;
6. publishes structured result evidence.

### 3. Keep the control plane trusted

The workflow definition/control plane must come from trusted/default-branch infrastructure.

The code under test is supplied separately as the requested exact SHA.

A PR/revision under test must not be able to silently redefine the privileged workflow that executes it.

### 4. Structured evidence

Every on-demand run exposes:

```text
revision
operation
target
canonical_command
result
run_id
run_url
```

plus useful failure logs.

A small JSON artifact plus GitHub job summary is sufficient.

### 5. Local convenience wrappers

Implement:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

For users with a checkout, these wrappers:

- resolve exact HEAD;
- reject relevant dirty state by default;
- prove HEAD is pushed;
- dispatch the generic workflow;
- capture the exact run identity;
- wait for it;
- surface failure logs;
- return its result.

Remote-only agents may instead dispatch via GitHub UI/API/CLI/connector without any local `./dev`.

### 6. Security/concurrency

Remote verification must:

- use read-only repository permissions by default;
- expose no release/deploy/write secrets;
- define authorized actors;
- define fork/untrusted-revision behavior;
- use job-private writable state;
- expose no arbitrary-shell input.

Do not cancel evidence for SHA A merely because SHA B is pushed. Verification identity is:

```text
revision + operation + target
```

not a mutable PR branch.

### M2 acceptance

A remote-only agent can:

1. discover a target through GitHub-visible repository state;
2. identify exact SHA A;
3. dispatch a focused test/verifier;
4. retrieve PASS/FAIL, logs, and run identity;
5. push SHA B and rerun;
6. never confuse SHA A evidence with SHA B.

And automatic PR CI remains only the cheap `./dev ci` gate.

---

## M3 — Converge verifier/oracle build ownership

### Goal

Eliminate the secondary normal build system under `reference/vespa` while preserving semantic behavior and independent oracle provenance.

### 1. Migrate active verifier compilation

Move the 55 active current-implementation sources into normal build authorities in small batches.

#### Rust — 49 sources

For each verifier family:

1. identify what it proves;
2. identify owning production crate(s);
3. move compiled source into an appropriate normal Cargo target;
4. preserve inputs/output;
5. point the semantic runner at the new target;
6. compare against the same frozen expectation;
7. delete the standalone workspace only after parity.

Possible homes:

- owning crate integration tests;
- normal test binaries;
- `searchkernel-testkit`;
- a small shared verifier-support crate only where real shared responsibility exists.

Do **not** create one giant verifier crate.

#### C — 4 sources

Move current native/ABI probes into explicit canonical native targets using canonical headers/compiler/ABI/runtime.

#### C++ — 2 sources

Move active SearchReply/current-implementation probes into the canonical native graph.

#### Uncertain fixture

For `m-api.rs`:

```text
prove active -> migrate
prove obsolete -> delete
```

### 2. Remove verifier-specific normal runtime setup

Once verifier binaries are normal targets, remove their ownership of:

- private normal native SDK construction;
- private adapter construction;
- verifier-specific compiler selection;
- verifier-specific Cargo target universes;
- ordinary `LD_LIBRARY_PATH` setup;
- duplicate normal runtime-closure assembly.

Allowed exceptions remain for:

- sanitizers;
- loader-negative tests;
- independent oracle execution;
- fixtures where environment isolation itself is the subject under test.

### 3. Make oracle authoring explicit only

Normal:

```bash
./dev verify ...
./dev universe
```

must consume committed frozen behavior.

Remove automatic/environment-detected live-oracle behavior.

Live oracle/golden generation becomes an explicit maintainer operation recording:

- pinned Vespa revision;
- oracle source;
- generation command;
- expected-output diff;
- provenance.

The 14 oracle sources remain independent and do not move merely for symmetry.

### 4. Reduce Universe to orchestration

`./dev universe` should only:

- enumerate canonical commands;
- invoke them;
- time/record them;
- continue after failures;
- summarize;
- print exact repro commands.

Example:

```text
FAIL: vespa.ranking
REPRO: ./dev verify vespa.ranking
```

Universe must not own compilers, SDK selection, adapter selection, or another runtime model.

### M3 acceptance

- all active current-implementation verifier binaries compile through normal Cargo/native authorities;
- semantic behavior/goldens are unchanged;
- normal verifier runners no longer construct private normal SDK/runtime universes;
- live oracle execution requires explicit maintainer intent;
- Universe invokes canonical commands only;
- every Universe failure has a focused repro command.

---

## M4 — Deliver ecosystem-native product distribution

### Goal

Make the end-user installation contract true:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

No supported managed-local user separately installs Vespa, `searchkerneld`, Search Kernel's dev images, or repository-specific SDK/runtime configuration.

### 1. Decide the native runtime packaging model

Preserve `searchkernel-ci-vespa` regardless; it is the development/build input.

Spike and compare:

#### A. Fully/static-per-consumer

Native closure linked into each consumer executable.

#### B. Mostly-static Search Kernel runtime

```text
libsearchkernel_vespa.so.2
    + adapter
    + Vespa absorbed statically
    + private dependencies static where practical
    + ordinary platform runtime libraries dynamic
```

#### C. Relocatable shared closure

```text
libsearchkernel_vespa.so.2
+ required Vespa/private shared libraries
```

Document:

- static archive availability;
- constructor/registry/dead-strip behavior;
- `--whole-archive` requirements;
- dynamic-only third-party dependencies;
- artifact size;
- link time/memory;
- relocatability;
- licensing/distribution implications;
- Cargo binary duplication;
- runtime identity and loader behavior.

Whatever model wins must pass an external-consumer proof with:

- no `LD_LIBRARY_PATH`;
- no manual Vespa install;
- no repository Cargo config;
- no developer SDK path;
- no cross-worktree/runtime-generation leakage.

Do not remove current #745 consumer-safe behavior until the replacement is proven.

### 2. Build one common managed-local runtime payload

For each supported platform/release, build one qualified payload:

```text
searchkerneld
libsearchkernel_vespa.so.2
required native runtime closure
runtime manifest/provenance
```

Reuse it for:

- standalone daemon archive;
- Python package;
- TypeScript/Node package;
- Ruby package.

The language packages must not independently rediscover/assemble native closures.

### 3. Python / pip

Target:

```bash
pip install searchkernel
```

Implement/qualify:

- platform wheel;
- common managed-local runtime payload;
- package-relative launcher discovery;
- managed-local default;
- optional connect mode;
- clean venv install/execution.

### 4. TypeScript/Node / npm

Target:

```bash
npm install @searchkernel/client
```

Use an idiomatic platform-runtime strategy. A likely shape:

```text
@searchkernel/client
    |
    +-- platform runtime dependency
          @searchkernel/runtime-linux-x64
          @searchkernel/runtime-linux-arm64
          ...
```

The user still installs only `@searchkernel/client`.

Qualify from a clean npm consumer project.

### 5. Ruby / RubyGems

Target:

```bash
gem install searchkernel
```

Implement/qualify:

- platform gem;
- common managed-local runtime assets;
- gemspec inclusion;
- managed-local lifecycle;
- optional connect mode;
- clean external `GEM_HOME`.

Ruby is not architecturally deferred merely because current packaging lags.

### 6. Rust / Cargo

Target:

```bash
cargo add searchkernel
cargo build
```

Cargo must transparently obtain/link/provision the chosen native runtime model.

The user must not:

- install Vespa;
- manually download the Search Kernel SDK;
- set `SK_NATIVE_SDK`;
- set `LD_LIBRARY_PATH`;
- use a repository path dependency.

Qualification uses a fresh external Cargo project against the actual publishable/distributable package.

### 7. Reconcile the support matrix

Update `docs/release-support.json` only after each artifact path is executable.

Intended first-party architecture:

```text
Rust            embedded native via Cargo
Python          managed-local + optional connect
TypeScript/Node managed-local + optional connect
Ruby            managed-local + optional connect
```

Unsupported platforms/surfaces remain explicit.

### M4 acceptance

On qualified platforms, all four commands are sufficient:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

and the resulting installed packages execute Search Kernel locally without a separate Vespa/searchkerneld install or dev-repository environment.

---

## M5 — Make release qualification and developer UX canonical

### Goal

Turn the converged build/distribution machinery into a coherent release and contributor experience.

### 1. Implement `./dev release-check`

Distinguish:

```text
./dev universe
    = exhaustive repository/source qualification

./dev release-check
    = build + qualify the actual artifacts intended for publication
```

For one exact revision, `release-check`:

1. asserts exact revision identity;
2. runs or consumes required exact-SHA source qualification;
3. resolves exact toolchain and Vespa/native image identities;
4. builds the qualified common runtime payload;
5. builds every supported product artifact;
6. writes release manifest/checksums/provenance;
7. installs/extracts every artifact into clean external environments;
8. executes every supported row in `docs/release-support.json`;
9. runs required ABI/persistence/generated-artifact/provenance checks;
10. fails closed if a supported row lacks executable evidence.

### 2. Artifact-level qualification

#### Rust

Fresh external Cargo project using the actual publishable/distributable Search Kernel package:

```text
add dependency
build
execute native operation
```

#### Python

```text
fresh venv
pip install candidate
execute managed-local operation
```

#### TypeScript/Node

```text
fresh project
npm install candidate
execute managed-local operation
```

#### Ruby

```text
fresh GEM_HOME
gem install candidate
execute managed-local operation
```

#### Standalone daemon

```text
extract into arbitrary clean location
clear repository/native overrides
launch/probe daemon
```

### 3. Release provenance

Manifest identifies at minimum:

- Search Kernel commit/version;
- platform/native target;
- toolchain image digest/identity;
- pinned Vespa commit;
- Vespa image/bundle identity;
- native packaging model/version;
- artifact checksums.

No release contract may depend on a historical workflow YAML filename/internal structure.

### 4. Simplify bootstrap

Normal supported host contract:

- Git;
- Docker/container runtime;
- GitHub/repository authentication;
- normal shell/filesystem/worktree support.

The canonical image owns Rust/C++/Python/Node/PHP/Ruby/FlatBuffers and normal build dependencies.

Converge `./dev doctor` toward validating/repairing the host boundary and ability to enter the canonical environment.

Host-native toolchain setup becomes advanced/optional, not the normal correctness definition.

### 5. Documentation and agent guidance

Rewrite `BUILDING.md` normal path to approximately:

```bash
./dev doctor
./dev build
./dev ci
./dev test --list
./dev test <scope>
./dev verify --list
./dev verify <lane>
./dev verify --remote <lane>
./dev universe
./dev release-check
```

Document advanced operations separately:

- image publishing;
- oracle authoring;
- host-native debugging;
- native distribution internals;
- release maintenance.

Update `AGENTS.md` with:

- local-capable verification protocol;
- remote-only workflow protocol;
- exact-SHA evidence requirements;
- no stale-run verification claims;
- focused checks before Universe;
- package/release verification expectations.

### M5 acceptance

- `./dev release-check` is sufficient evidence that one exact revision's advertised artifacts are qualified;
- every supported support-matrix row has executable artifact evidence;
- a new contributor can understand the normal build workflow from roughly one screen of documentation;
- normal bootstrap does not require host language/native toolchains.

---

## M6 — Delete duplicate authorities and perform final adversarial qualification

### Goal

Remove compatibility machinery only after the converged system is proven, then test the resulting system against every persona.

### 1. Delete obsolete paths

Candidates include:

- standalone active verifier Cargo workspaces;
- duplicate verifier lockfiles;
- verifier-specific normal SDK assembly;
- verifier-specific normal runtime setup;
- ad-hoc current-implementation C/C++ compiler invocations;
- duplicated target registries;
- obsolete resolver paths;
- stale workflow-internal contract tests;
- stale release-lane YAML coupling;
- compatibility launchers made unnecessary by the chosen runtime packaging;
- old normal host-toolchain bootstrap behavior.

For every deletion answer:

> **What canonical component now owns this responsibility?**

If the answer is unclear, do not delete it.

### 2. Explicitly protect

Do not delete or accidentally flatten:

- direct oracle sources;
- committed goldens/source traces/reference fixtures;
- the toolchain/Vespa/thin-leaf image split;
- strong specialist verifier availability;
- package-manager-native product distribution;
- remote-only verification.

### 3. Persona acceptance

#### Bootstrapper

Fresh supported machine:

```text
clone
./dev doctor
./dev build
```

No undocumented language/native setup.

#### Local implementer

- warm focused edit/test loop;
- target discovery works;
- worktree isolation proven.

#### Remote-only coding agent

No local Search Kernel environment:

- discover target;
- dispatch exact-SHA test/verifier;
- retrieve structured result/logs;
- rerun after a new commit.

#### Reviewer

- independently run/dispatch adversarial verification against exact PR head;
- prove run SHA;
- distinguish cheap PR gate from specialist verification and release qualification.

#### End user

Clean external consumers prove:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

with managed-local execution and no separate Vespa/searchkerneld installation.

#### Release maintainer

```bash
./dev release-check
```

builds and qualifies every advertised release surface from actual artifacts.

#### CI / automation

- PR CI is thin;
- remote workflow bounded;
- runner trust policy enforced;
- no hidden shared mutable state.

#### Oracle maintainer

- normal verification never authors truth;
- explicit oracle generation remains available and provenance-preserving.

### 4. Adversarial checks

Try to falsify:

- cross-worktree native artifact contamination;
- stale Cargo target reuse;
- stale remote evidence after a new push;
- mutable-branch ambiguity;
- untrusted revision changing the remote control plane;
- Vespa image drift from thin-leaf provenance;
- release packages shipping a different runtime than was qualified;
- package-manager installs missing runtime assets;
- `LD_LIBRARY_PATH` making a broken package appear healthy;
- verifier dependence on private SDK setup;
- automatic oracle execution;
- support-matrix claims without executable artifact evidence.

### M6 acceptance

Every Definition-of-Done item in `implementation-plan/build-system.md` is true on the same accepted revision.

Only after that is the convergence complete.

---

## 4. Issue / PR decomposition

The six milestones are roadmap units, **not** PR units.

Each should be implemented through small issues/PRs.

### M1 issue families

- build-system inventory;
- target registry;
- `build` / `ci` / listing commands;
- image-provenance authority tests;
- worktree/cache ownership.

### M2 issue families

- CI delegates to `./dev ci`;
- generic on-demand workflow;
- trusted-main control-plane rule;
- structured result artifact;
- `./dev --remote` wrappers;
- security/trust regression tests;
- remote-agent documentation.

### M3 issue families

- Rust verifier migrations by family;
- C/C++ current-implementation probe migration;
- shared normal runtime path;
- removal of verifier SDK duplication;
- explicit oracle authoring;
- Universe orchestration/repro.

### M4 issue families

- native static/shared runtime spike + ADR;
- common managed-local runtime payload;
- Python pip distribution;
- TypeScript npm distribution/runtime packages;
- Ruby platform gem distribution;
- Rust Cargo distribution;
- support-matrix reconciliation.

### M5 issue families

- release manifest/provenance;
- package build orchestration;
- clean-install E2E per ecosystem;
- `release-check`;
- doctor simplification;
- BUILDING.md / AGENTS.md convergence.

### M6 issue families

- deletion PRs grouped by removed authority;
- final persona acceptance audit;
- separate remediation PRs for defects found by that audit.

---

## 5. Hard merge gates

### Gate A — before deleting a verifier build path

Must have:

- canonical replacement target;
- semantic runner using it;
- unchanged frozen behavior;
- exact verifier passing.

### Gate B — before deleting native runtime compatibility behavior

Must have:

- chosen/proven M4 native model;
- external consumer passing without repository/native environment;
- fail-closed runtime identity.

### Gate C — before declaring an ecosystem supported

Must have normal package-manager install + managed-local execution from the actual distributable artifact.

### Gate D — before making `release-check` authoritative

Must have:

- reconciled `docs/release-support.json`;
- executable evidence for every supported row;
- release manifest/provenance;
- no stale CI-YAML authority.

### Gate E — before removing normal host-toolchain bootstrap

Docker-backed canonical flow must already support:

- build;
- ci;
- test;
- verify;
- remote dispatch;
- release qualification where relevant.

---

## 6. Explicit non-goals

Do not turn this roadmap into:

- a Bazel/Nix migration;
- a `cargo xtask` rewrite;
- a verifier DSL;
- a generalized remote shell;
- a reporting-dashboard project;
- a rewrite of semantic verifier expectations;
- a rewrite of the native API;
- an expansion to uncommitted release platforms;
- a reason to rebuild Vespa on every Search Kernel commit.

The objective is a boring system with obvious ownership, reusable immutable heavy inputs, local and remote verification, and native package-manager installation for end users.

---

## 7. Final completion checklist

The roadmap is complete only when all are true:

- `./dev` is the local semantic authority;
- GitHub on-demand verification is a first-class bounded interface for remote-only agents;
- automatic PR CI delegates to `./dev ci`;
- toolchain image, Vespa/native image, and thin leaf remain independent;
- ordinary Search Kernel source changes never republish Vespa;
- worktrees retain useful warm state without shared mutable-target contamination;
- active verifier code compiles through normal build authorities;
- normal verifier runners do not create private normal SDK/runtime universes;
- live oracle execution is explicit only;
- Universe is orchestration only;
- native consumer packaging has one deliberately chosen model;
- the common managed-local runtime payload is built once per platform/release;
- Python, TypeScript/Node, and Ruby consume that same qualified payload;
- Rust obtains the chosen native model transparently through Cargo;
- `cargo add searchkernel` works;
- `pip install searchkernel` works;
- `npm install @searchkernel/client` works;
- `gem install searchkernel` works;
- supported managed-local users do not separately install Vespa or `searchkerneld`;
- `./dev release-check` qualifies actual artifacts;
- release provenance ties artifacts to exact Search Kernel/toolchain/Vespa identities;
- duplicate authorities are deleted only after replacement proof;
- every persona acceptance scenario passes on one exact final revision.
