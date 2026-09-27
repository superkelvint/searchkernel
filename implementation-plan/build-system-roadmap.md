# Search Kernel Build-System Convergence Roadmap

Status: proposed implementation roadmap  
Design authority: `implementation-plan/build-system.md`  
Scope: implementation sequencing only; this document does not redefine the architecture/PRD

## 1. Purpose

`build-system.md` defines the desired build-system architecture, personas, distribution model, and acceptance criteria.

This roadmap answers a different question:

> **In what order do we change the repository so that the desired architecture becomes true without breaking existing developer, agent, verifier, or consumer workflows along the way?**

The migration is intentionally incremental. Existing working paths are removed only after a canonical replacement is executable and verified.

The primary implementation principles are:

1. **Preserve working behavior before cleanup.**
2. **Create the canonical authority before deleting duplicate implementations.**
3. **Keep automatic PR CI small.**
4. **Keep heavy verification available explicitly, including to remote-only coding agents.**
5. **Preserve the current three-axis Docker image architecture.**
6. **Do not rebuild or republish Vespa for ordinary Search Kernel source changes.**
7. **Treat package-manager-native installation as the end-user contract.**
8. **Tie review and release evidence to exact commits and actual distributable artifacts.**
9. **Do not collapse oracle authoring into normal verification.**
10. **One issue / one coherent change / one reviewable PR wherever practical.**

---

## 2. Target end state

The target architecture is:

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
                        canonical test/verifier
                             semantics


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

qualified common native runtime payload
              |
              +--> standalone searchkerneld archive
              +--> Python managed-local package
              +--> TypeScript/Node managed-local package
              +--> Ruby managed-local package

Rust embedded distribution
              |
              v
normal Cargo package mechanics transparently
provide/link/provision the native runtime

User installation surfaces:

    cargo add searchkernel
    pip install searchkernel
    npm install @searchkernel/client
    gem install searchkernel
```

---

## 3. Delivery strategy

The work is divided into milestones with hard exit criteria.

A milestone is complete only when:

- its new authority exists;
- its regression/contract coverage exists;
- relevant existing behavior is preserved;
- the exact acceptance commands are recorded;
- follow-on deletion is explicitly safe.

Do not combine multiple milestones into one large PR merely because they touch `./dev`, CI, or Docker.

### Milestone dependency graph

```text
M0  Freeze + inventory
 |
 +--> M1 Canonical command/target authorities
 |      |
 |      +--> M2 Minimal CI delegates to ./dev
 |      |
 |      +--> M4 Remote-only verification
 |
 +--> M3 Preserve/formalize Docker image architecture
 |      |
 |      +--> M5 Native runtime distribution decision
 |              |
 |              +--> M9 Ecosystem-native product distribution
 |
 +--> M6 Verifier build-graph migration
 |      |
 |      +--> M7 Collapse verifier SDK/runtime duplication
 |              |
 |              +--> M8 Oracle + Universe convergence
 |
 +----------------------------------------------------+
                                                      |
                                                      v
                                             M10 release-check
                                                      |
                                                      v
                                             M11 bootstrap/docs
                                                      |
                                                      v
                                             M12 delete legacy paths
                                                      |
                                                      v
                                             M13 final qualification
```

M1, M3, and parts of M5 may proceed in parallel after M0.

---

# Milestones

## M0 — Freeze the current build-system inventory

### Goal

Create a precise map of the existing build, test, verifier, image, SDK, runtime, packaging, and CI authorities before changing ownership.

### Why first

The current build system contains several working mechanisms that look redundant but encode important invariants. In particular, the three-image Docker model and existing native runtime packaging must not be accidentally removed as "cleanup."

### Deliverables

Create a checked-in inventory under:

```text
implementation-plan/audit/build-system/
```

covering at minimum:

- all public `./dev` operations;
- every direct Cargo invocation outside `./dev`;
- every compiler/interpreter resolver;
- every Docker launcher;
- all Docker image publishers;
- all Docker image consumers;
- all native SDK resolvers/builders;
- all native adapter/runtime staging;
- all `LD_LIBRARY_PATH`, RUNPATH, RPATH, SONAME, and runtime-launcher logic;
- all standalone verifier Cargo workspaces;
- active C/C++ verifier compilation;
- all direct oracle compilation/execution;
- all GitHub Actions workflows;
- all release/package builders;
- all current release support rows;
- all package-manager artifacts;
- all current external-consumer tests.

Each item is classified as:

```text
CANONICAL AUTHORITY
CALLER
TEMPORARY COMPATIBILITY
ORACLE AUTHORING
DEAD / SUSPECTED DEAD
```

### Required explicit baseline records

Record the current image architecture:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

and the relationship between them.

Record the current active verifier classification:

```text
55 active current-implementation compiled sources
14 direct oracle/golden-authoring sources
1 uncertain compile fixture
```

### Acceptance

A reviewer can answer, from the inventory alone:

- who owns the canonical toolchain;
- who owns the Vespa payload;
- how the native adapter is built;
- how local/CI execution receives Vespa;
- how each verifier is compiled;
- how each supported product artifact is assembled;
- which parts are safe candidates for deletion.

### Do not do yet

- no verifier migration;
- no resolver deletion;
- no image flattening;
- no runtime packaging redesign.

---

## M1 — Establish one canonical command and target registry

### Goal

Make the public build/test/verifier vocabulary explicit and machine-readable before adding more callers.

### Deliverables

Introduce one repository-owned registry for public test/verifier targets.

Conceptually:

```text
build-system target registry
    |
    +-- tests
    |    +-- rust
    |    +-- python
    |    +-- typescript
    |    +-- ruby
    |    +-- ...
    |
    +-- verifiers
         +-- persistence
         +-- vespa.ranking
         +-- vespa.grouping.yql
         +-- ...
```

The representation may be Rust, JSON/TOML, or generated repository data, but there must be **one authority**.

It must drive:

```bash
./dev test --list
./dev verify --list
./dev test <scope>
./dev verify <lane>
```

and later drive remote workflow validation.

### Add canonical top-level operations

Implement or normalize:

```bash
./dev build
./dev ci
./dev test --list
./dev verify --list
```

Definitions:

- `build` = canonical supported product build in the canonical environment;
- `ci` = cheap mandatory PR gate;
- `test --list` / `verify --list` = finite supported vocabulary.

### Change-area guidance

Add lightweight documentation mapping common change areas to focused checks.

Example:

```text
core/crates/searchkernel-ir/**  -> test rust
core/native/**                  -> relevant native/verifier lanes
clients/python/**               -> test python
clients/typescript/**           -> test typescript
clients/ruby/**                 -> test ruby
persistence-format changes      -> verify persistence
```

This is guidance, not dependency analysis.

### Acceptance

- local callers and future remote callers can discover targets from one source;
- invalid targets fail closed;
- `./dev` does not maintain a second hard-coded target list;
- the registry is readable through GitHub for agents with no development environment.

---

## M2 — Make automatic PR CI a thin `./dev ci` caller

### Goal

Finish the intentional CI simplification and make the repository command authoritative.

### Target PR gate

```text
cargo fmt --check
cargo check
cargo clippy -D warnings
```

with exact flags defined once by `./dev ci`.

### Deliverables

Change automatic PR CI to conceptually:

```text
checkout exact PR revision
./dev ci
```

GitHub Actions should not independently own:

- Cargo flag selection;
- compiler selection;
- native SDK setup;
- verifier selection;
- test semantics.

### Contract-test cleanup

Update repository tooling tests so they validate:

- `./dev ci` behavior;
- workflow delegation;
- exact-head execution;
- isolation/security invariants;

rather than hard-coding removed workflow names or obsolete YAML internals.

### Acceptance

Every automatic CI failure has one local reproduction command:

```bash
./dev ci
```

Heavy verifiers are **not** restored to mandatory PR CI.

---

## M3 — Preserve and formalize the three-axis Docker image architecture

### Goal

Make the image model an explicit implementation authority instead of incidental tooling.

### Preserve these independent identities

#### Toolchain

```text
searchkernel-ci-toolchain:<toolchain-id>
```

Contains:

- OS;
- Rust/Cargo;
- GCC/C++;
- Python;
- Node;
- PHP;
- Ruby;
- FlatBuffers;
- reusable generation/client tooling;
- pinned sccache tooling.

#### Vespa/native input

```text
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
```

Contains:

- pinned Vespa native payload;
- required headers;
- build/link metadata;
- native provenance.

This remains the only development image carrying the large Vespa payload.

#### Thin Search Kernel leaf

```text
searchkernel-ci:<searchkernel-revision>
```

The leaf:

- records the exact toolchain identity;
- records the exact Vespa image digest;
- validates compatibility;
- does not copy Vespa into the committed layer.

### Preserve publication commands

```bash
./dev image toolchain publish
./dev image vespa publish
./dev image publish
```

### Runtime materialization

Keep the digest-addressed Vespa payload materialization:

```text
leaf -> exact Vespa digest
         |
         v
content-addressed local Docker volume
         |
         v
read-only mount at /opt/searchkernel/native-sdk-cache
```

### Local mutable build-state decision

Implement the desired warm local behavior without losing worktree isolation.

Target:

- immutable Vespa/toolchain inputs shared by identity;
- sccache shared as non-authoritative content cache;
- writable Cargo/native incremental outputs persistent **per worktree + compatible environment identity**;
- no writable Cargo target tree shared across unrelated worktrees/agents;
- CI writable targets remain job-private.

If measurement shows persistent Cargo targets are not materially better than private targets + sccache, document the evidence and explicitly amend the architecture before deviating.

### Acceptance

- normal Search Kernel source publication does not upload Vespa bytes;
- toolchain-only changes do not republish Vespa;
- Vespa updates create a new immutable native identity;
- leaf provenance points to immutable toolchain/Vespa identities;
- local and CI execution resolve the same identities;
- concurrent worktrees cannot mutate the same Cargo target tree.

---

## M4 — Add first-class on-demand verification for remote-only agents

### Goal

Allow coding agents with GitHub access but **no usable development environment** to run targeted tests/verifiers outside normal automatic CI.

### Control-plane rule

The trusted workflow definition lives on trusted/default-branch infrastructure.

The code under test is supplied separately as:

```text
revision = exact pushed SHA
```

The requested revision must not be allowed to redefine the privileged workflow control plane used to execute it.

### Generic workflow

Add one bounded on-demand workflow:

```text
operation = ci | test | verify
target    = known scope/lane where applicable
revision  = exact commit SHA
```

The workflow:

1. validates the caller/trust policy;
2. validates the requested operation/target;
3. checks out the exact requested SHA;
4. verifies `git rev-parse HEAD == requested SHA`;
5. executes:
   - `./dev ci`, or
   - `./dev test <scope>`, or
   - `./dev verify <lane>`;
6. publishes structured result evidence.

### Structured result evidence

Every run should expose at least:

```text
revision
operation
target
canonical_command
result
run_id
run_url
```

A small JSON artifact and GitHub job summary are sufficient.

### Local convenience wrappers

Implement:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

For callers with a checkout these wrappers:

- resolve exact HEAD;
- reject relevant dirty state by default;
- verify HEAD is pushed;
- dispatch the same workflow;
- capture the exact run;
- wait;
- surface failed logs;
- propagate result.

### Remote-only caller

A remote-only agent may dispatch through:

- GitHub Actions UI;
- GitHub API;
- GitHub CLI;
- an authorized GitHub connector/tool.

No local `./dev` is required.

### Security

The workflow must:

- use read-only repository permissions by default;
- receive no release/deploy/write secrets;
- have an explicit authorized-actor policy;
- define behavior for fork/untrusted revisions;
- use job-private writable state;
- avoid a generic arbitrary-shell input.

### Concurrency

Do not cancel a run merely because a PR receives a newer commit.

Verification evidence belongs to:

```text
revision + operation + target
```

not to a mutable PR branch.

### Acceptance

An authorized agent with only GitHub access can:

1. discover valid targets;
2. push/identify SHA A;
3. request `verify vespa.ranking @ SHA A`;
4. receive PASS/FAIL + logs + run URL;
5. push SHA B;
6. rerun;
7. never confuse SHA A evidence with SHA B.

---

## M5 — Decide and prove the end-user native runtime packaging model

### Goal

Resolve the P0 question: how the embedded Vespa implementation reaches downstream users.

### Preserve the development image model

This milestone **does not remove** `searchkernel-ci-vespa`.

That image remains the build input regardless of the final consumer package layout.

### Spike the candidate models

At minimum compare:

#### A. Fully/static-per-consumer

Native closure linked into each consumer executable.

#### B. Mostly-static Search Kernel runtime

```text
libsearchkernel_vespa.so.2
    + Search Kernel adapter
    + Vespa implementation absorbed statically
    + private dependencies static where practical
    + ordinary platform runtime dependencies dynamic
```

#### C. Relocatable shared closure

```text
libsearchkernel_vespa.so.2
+ required Vespa/private shared libraries
```

### Required evidence

Measure/document:

- static archive availability;
- static constructor/registry behavior;
- dead stripping / `--whole-archive` needs;
- third-party libraries that cannot be static;
- artifact size;
- link time/memory;
- loader behavior;
- relocatability;
- license/distribution implications;
- external-consumer runtime identity;
- impact on Cargo test binary duplication.

### Required external consumer proof

Whatever model wins must prove, outside the repository:

- no `LD_LIBRARY_PATH`;
- no manually installed Vespa;
- no repository Cargo config;
- no accidental adapter from another worktree/build;
- no developer SDK path dependency.

### Decision artifact

Add a short ADR or decision record containing:

```text
chosen model
rejected models
measured evidence
runtime layout
provenance rules
platform assumptions
```

### Gate

Do not remove the current #745 consumer-safe behavior until the replacement passes this acceptance.

---

## M6 — Migrate active verifier compilation into the normal build graph

### Goal

Remove the second compilation architecture under `reference/vespa` while preserving verifier semantics.

This is a build-ownership migration, not a behavioral rewrite.

### M6A — Rust verifier migration

Migrate the 49 active Rust sources in small batches.

For each verifier:

1. identify what it proves;
2. identify the owning production crate(s);
3. create an appropriate normal Cargo target;
4. preserve inputs/output exactly;
5. point the semantic verifier runner to the new target;
6. compare against the same frozen expected result;
7. delete the standalone workspace only after parity.

Possible homes:

- owning crate integration tests;
- normal test binaries;
- `searchkernel-testkit`;
- a small verifier-support crate only where real shared ownership exists.

Do not create one giant "all verifiers" crate.

### M6B — C native probes

Move the four active C probes into explicit canonical native test/verifier targets.

They must inherit:

- canonical header;
- canonical compiler/toolchain;
- canonical native ABI;
- canonical adapter/runtime.

### M6C — C++ current-implementation probes

Move the two active SearchReply/current-implementation C++ probes into the canonical native graph.

### M6D — uncertain API fixture

For `m-api.rs`:

```text
prove active -> migrate
prove obsolete -> delete
```

### Acceptance per migrated lane

The old and new implementations produce equivalent PASS/FAIL behavior against unchanged fixtures/goldens.

### Completion criterion

All active verifier programs compile through normal Cargo/native build authorities.

---

## M7 — Remove verifier-specific SDK/runtime duplication

### Goal

Once verifier binaries are normal targets, make them use the same native runtime model as the product.

### Remove normal-path verifier ownership of

- private native SDK construction;
- private adapter construction;
- verifier-specific compiler selection;
- verifier-specific Cargo target universes;
- normal `LD_LIBRARY_PATH` setup;
- duplicate runtime closure assembly.

### Allowed exceptions

Keep specialized behavior only for:

- sanitizers;
- loader-negative tests;
- intentionally independent oracle execution;
- narrow compatibility fixtures where isolation is the subject under test.

### Acceptance

A semantic verifier runner may:

- prepare scenarios;
- allocate temp dirs;
- invoke a canonical target;
- compare output;
- report PASS/FAIL.

It may not create its own normal Search Kernel build/runtime universe.

---

## M8 — Separate oracle authoring and reduce Universe to orchestration

### Goal

Make the distinction executable:

> Oracle generates truth. Verifier checks against committed truth.

### M8A — explicit oracle authoring

Remove normal `auto` live-oracle behavior.

Normal:

```bash
./dev verify ...
./dev universe
```

must consume committed frozen behavior only.

Live oracle/golden generation requires an explicit maintainer operation.

The operation must record:

- pinned Vespa revision;
- oracle source;
- generation command;
- resulting golden diff;
- provenance metadata.

### M8B — Universe

`./dev universe` becomes orchestration only.

It:

- enumerates canonical checks;
- runs them;
- records duration/result;
- does not fail-fast;
- prints exact repro commands.

Example:

```text
FAIL: vespa.ranking
REPRO: ./dev verify vespa.ranking
```

Universe must not choose a compiler, SDK, adapter, or independent runtime model.

### Acceptance

- normal verification never regenerates expected behavior;
- Universe runs only canonical test/verifier commands;
- every Universe failure has an exact focused repro.

---

## M9 — Deliver ecosystem-native product installation

### Goal

Make the supported end-user installation contract true:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

No supported managed-local installation requires:

- separate Vespa installation;
- separate `searchkerneld` installation;
- Search Kernel development Docker images;
- `./dev`;
- manual SDK download;
- repository-local configuration;
- ambient loader hacks.

### M9A — one qualified common managed-local runtime payload

Build one platform-specific payload:

```text
searchkerneld
libsearchkernel_vespa.so.2
required native runtime closure
runtime manifest/provenance
```

This payload is assembled once per platform/release and reused by:

- standalone daemon archive;
- Python package;
- TypeScript/Node package;
- Ruby package.

Language packages must not independently rediscover or assemble the native closure.

### M9B — Python / pip

Target UX:

```bash
pip install searchkernel
```

Implement/verify:

- platform wheel;
- packaged common runtime payload;
- package-relative launcher discovery;
- managed-local default;
- optional connect mode;
- clean environment install test.

Acceptance:

```text
fresh venv
pip install built/published candidate
Python API opens managed-local Search Kernel
no repository/native environment variables
```

### M9C — TypeScript / npm

Target UX:

```bash
npm install @searchkernel/client
```

Choose an idiomatic runtime packaging shape.

A likely scalable model:

```text
@searchkernel/client
    + optional/platform dependency:
      @searchkernel/runtime-linux-x64
      @searchkernel/runtime-linux-arm64
      ...
```

The user still installs only the client package.

Implement/verify:

- platform runtime selection;
- managed-local daemon lifecycle;
- optional connect mode;
- clean consumer install from packed/publishable artifacts.

### M9D — Ruby / RubyGems

Target UX:

```bash
gem install searchkernel
```

Implement:

- platform gem strategy;
- package-relative common runtime assets;
- gemspec inclusion;
- managed-local daemon lifecycle;
- optional connect mode;
- clean external `GEM_HOME` qualification.

Ruby is not architecturally deferred merely because current packaging lags.

### M9E — Rust / Cargo

Target UX:

```bash
cargo add searchkernel
cargo build
```

The Cargo package must transparently obtain/link/provision the chosen M5 runtime model.

The user must not:

- install Vespa;
- download the Search Kernel SDK manually;
- set `SK_NATIVE_SDK`;
- set `LD_LIBRARY_PATH`;
- use a repository path dependency.

Acceptance:

```text
new cargo project
dependency on actual publishable Search Kernel artifact
cargo build
run native Search Kernel operation
clean runtime environment
```

### M9F — reconcile support matrix

Update `docs/release-support.json` only after the corresponding artifact path is executable.

Intended first-party client architecture:

```text
Python          managed-local + optional connect
TypeScript/Node managed-local + optional connect
Ruby            managed-local + optional connect
Rust            embedded native via Cargo
```

Unsupported platform rows remain explicit.

---

## M10 — Implement `./dev release-check` as shipability authority

### Goal

Separate repository correctness from actual artifact shipability.

### Distinction

```text
./dev universe
    = exhaustive repository/source qualification

./dev release-check
    = build + qualify the actual artifacts to be published
```

### Release-check pipeline

For one exact revision:

1. assert exact revision identity;
2. require or run required source qualification;
3. resolve exact toolchain image identity;
4. resolve exact Vespa/native image identity;
5. build the release-native runtime payload;
6. build every supported product artifact;
7. write release manifest and checksums;
8. install/extract every artifact into clean external environments;
9. execute every supported row in `docs/release-support.json`;
10. run ABI/provenance/persistence/generated-artifact checks;
11. fail closed if any supported row lacks executable evidence.

### Required artifact qualification

#### Rust

Actual publishable/distributable Cargo package:

```text
create clean external Cargo project
install/depend through normal Cargo package mechanism
build
execute native operation
```

#### Python

```text
fresh venv
pip install candidate artifact
execute managed-local operation
```

#### TypeScript/Node

```text
fresh project
npm install candidate artifact
execute managed-local operation
```

#### Ruby

```text
fresh GEM_HOME
gem install candidate artifact
execute managed-local operation
```

#### Standalone daemon

```text
extract tarball to arbitrary clean location
clear repository/native overrides
launch/probe daemon
```

### Provenance

The release manifest must identify at minimum:

- Search Kernel commit;
- Search Kernel version;
- platform/native target;
- toolchain image digest/identity;
- pinned Vespa commit;
- Vespa image/bundle identity;
- native packaging model/version;
- artifact checksums.

### Acceptance

A successful `./dev release-check` is sufficient evidence for:

> This exact revision's advertised supported artifacts were built and qualified.

No release contract may depend on the internal name/content of a historical GitHub workflow YAML file.

---

## M11 — Simplify bootstrap and developer documentation

### Goal

Make the normal contributor model small and understandable.

### Supported host contract

Normal development requires only:

- Git;
- Docker/container runtime;
- GitHub/repository authentication;
- normal shell/filesystem/worktree support.

The canonical image owns:

- Rust;
- C/C++;
- Python;
- Node;
- PHP;
- Ruby;
- FlatBuffers;
- normal build dependencies.

### `./dev doctor`

Converge `doctor` toward validating/repairing:

- Docker availability;
- repository access;
- image availability/auth;
- Git/worktree assumptions;
- ability to enter the canonical environment.

Host-native language/toolchain installation becomes advanced/optional, not the normal correctness definition.

### Documentation

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

Document separately:

- image publishing;
- oracle authoring;
- host-native debugging;
- native distribution internals;
- release maintenance.

### Agent guidance

Update `AGENTS.md` with:

- local-capable verification protocol;
- remote-only verification protocol;
- exact-SHA evidence requirements;
- no claim of verification from stale runs;
- use focused checks before Universe;
- package/release verification expectations where relevant.

---

## M12 — Delete compatibility and duplicate authority

### Goal

Remove machinery whose responsibility has successfully moved.

### Candidate deletions

Only after the replacement is proven:

- standalone active verifier Cargo workspaces;
- duplicate verifier lockfiles;
- verifier-specific normal SDK assembly;
- verifier-specific normal runtime setup;
- ad-hoc current-implementation C/C++ compiler invocations;
- duplicated target registries;
- obsolete resolver paths;
- stale workflow-contract tests;
- stale release-lane YAML coupling;
- compatibility launchers made unnecessary by the chosen runtime packaging;
- old host bootstrap behavior no longer part of supported normal development.

### Mandatory deletion question

For every deleted component:

> **What canonical component now owns this responsibility?**

If the answer is not obvious, do not delete it.

### Special protection

Do not delete:

- direct oracle sources merely because they are unusual;
- the three-axis Docker image architecture;
- source traces/goldens/reference fixtures;
- strong specialist verifier availability;
- product packaging paths required by package-manager-native installation.

---

## M13 — Final adversarial qualification

### Goal

Prove the new build system satisfies every persona rather than merely passing its own happy-path tests.

### Persona acceptance matrix

#### Bootstrapper

Fresh supported machine:

```text
clone
./dev doctor
./dev build
```

No undocumented language/native setup.

#### Local implementer

Repeated focused build/test cycle:

- warm behavior measured;
- worktree isolation proven;
- focused test/verifier discovery works.

#### Remote-only coding agent

No local Search Kernel environment:

- discover target;
- dispatch exact-SHA test/verifier;
- retrieve structured result/logs;
- rerun after new commit.

#### Reviewer

For exact PR head:

- independently dispatch adversarial verifier;
- prove run SHA;
- distinguish PR gate from specialist verification and release qualification.

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

- automatic PR gate is thin;
- remote workflow is bounded;
- self-hosted runner trust policy enforced;
- no hidden mutable cross-job state.

#### Oracle maintainer

- normal verifier never authors truth;
- explicit oracle generation remains independently available and provenance-preserving.

### Adversarial checks

Specifically attempt to falsify:

- cross-worktree native artifact contamination;
- stale Cargo target reuse;
- stale remote verification evidence after a new push;
- mutable branch ambiguity;
- untrusted revision changing remote-control behavior;
- Vespa image drift from leaf provenance;
- release package using a different native runtime than qualification;
- package-manager install missing runtime assets;
- `LD_LIBRARY_PATH` accidentally making a broken package look healthy;
- verifier still depending on private SDK setup;
- automatic oracle execution;
- release support matrix claiming a surface without executable artifact evidence.

### Completion criterion

All Definition-of-Done items in `implementation-plan/build-system.md` are demonstrably true on the same accepted revision.

---

# 4. Recommended issue / PR decomposition

Each milestone should be a tracker, not one implementation PR.

Suggested decomposition:

## M0

- inventory commands/toolchains/images;
- inventory verifier build paths;
- inventory native runtime/package paths;
- inventory CI/release contracts.

## M1

- canonical target registry;
- `test --list`;
- `verify --list` migration;
- `build` command;
- `ci` command;
- check-selection documentation.

## M2

- CI delegates to `./dev ci`;
- remove stale workflow-internal assertions;
- exact-head CI regression tests.

## M3

- image provenance authority tests;
- axis-specific publish contract tests;
- worktree-scoped local target cache;
- local/CI image identity parity tests.

## M4

- generic workflow;
- structured result artifact;
- trusted-main control-plane rule;
- `./dev --remote` wrappers;
- remote target discovery;
- agent documentation;
- remote security regression tests.

## M5

- static/shared runtime spike;
- decision record;
- external-consumer prototype;
- runtime identity/relocatability tests.

## M6

Create small migration PRs by verifier family rather than one 55-file move.

## M7

- common native runtime path for verifier targets;
- remove verifier SDK duplication in batches;
- delete verifier target/cache special cases.

## M8

- disable automatic live oracle;
- explicit oracle command;
- Universe canonical-command migration;
- exact-repro summary.

## M9

- common managed-local runtime payload;
- Python package qualification;
- Node package/runtime split if chosen;
- Ruby platform gem/runtime packaging;
- Rust Cargo distribution;
- support-matrix reconciliation.

## M10

- release manifest/provenance;
- package build orchestration;
- per-surface install/run qualification;
- `release-check` aggregation.

## M11

- doctor simplification;
- BUILDING.md rewrite;
- AGENTS.md remote/local protocols;
- remove normal host-toolchain dependency.

## M12

Delete obsolete paths in small reviewable PRs grouped by authority removed.

## M13

One final adversarial audit PR/issue that contains evidence only; functional fixes found by the audit should be separate remediation PRs.

---

# 5. Merge gates

## Gate A — before deleting verifier build paths

Must have:

- canonical target exists;
- semantic runner uses it;
- frozen behavior unchanged;
- exact verifier passes.

## Gate B — before deleting native runtime compatibility paths

Must have:

- M5 chosen runtime model;
- external consumer passes without repository/native environment;
- package/runtime identity is fail-closed.

## Gate C — before declaring a language surface supported

Must have clean package-manager install + managed-local execution from actual distributable artifact.

## Gate D — before making `release-check` authoritative

Must have:

- reconciled `docs/release-support.json`;
- executable evidence for every supported row;
- release manifest/provenance;
- no stale CI-YAML dependencies.

## Gate E — before removing old host bootstrap

Must have Docker-backed:

- build;
- ci;
- test;
- verify;
- remote dispatch;
- release qualification where relevant.

---

# 6. Things explicitly not to optimize during this roadmap

Do not turn this effort into:

- a Bazel/Nix migration;
- a `cargo xtask` rewrite;
- a verifier DSL;
- a generalized remote shell;
- a reporting dashboard project;
- a rewrite of semantic verifier expectations;
- a rewrite of the native API;
- a multi-platform release expansion beyond the explicitly supported matrix;
- a reason to rebuild Vespa for every Search Kernel commit.

The objective is a build system whose ownership is obvious, whose heavy native inputs are reused deliberately, whose tests are reachable by both local and remote-only agents, and whose end-user installation feels native to Cargo/npm/pip/RubyGems.

---

# 7. Final completion checklist

The roadmap is complete only when all of the following are simultaneously true:

- `./dev` is the local semantic authority;
- GitHub on-demand verification is a first-class bounded interface for remote-only agents;
- automatic PR CI delegates to `./dev ci`;
- the toolchain image, Vespa/native image, and thin leaf remain independent;
- ordinary Search Kernel source changes never republish Vespa;
- local worktrees retain useful warm state without shared mutable target contamination;
- active verifier code is compiled by normal build authorities;
- normal verifier runners do not construct private normal SDK/runtime universes;
- live oracle execution is explicit only;
- Universe is orchestration only;
- native consumer packaging has one deliberately chosen model;
- the common managed-local runtime payload is built once per platform/release;
- Python, TypeScript/Node, and Ruby consume that same qualified payload;
- Rust consumes the chosen native model transparently through Cargo;
- `cargo add searchkernel` works;
- `pip install searchkernel` works;
- `npm install @searchkernel/client` works;
- `gem install searchkernel` works;
- no supported managed-local user separately installs Vespa or `searchkerneld`;
- `./dev release-check` qualifies actual artifacts;
- release provenance ties artifacts to Search Kernel/toolchain/Vespa identities;
- obsolete duplicate authorities are deleted only after replacement proof;
- all persona acceptance scenarios pass on one exact final revision.
