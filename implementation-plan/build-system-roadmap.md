# Search Kernel Build-System Convergence Roadmap

Status: proposed implementation roadmap  
Design authority: `implementation-plan/build-system.md`  
Scope: implementation sequencing only; this document does not redefine the architecture/PRD

## 1. Purpose

`build-system.md` is the design contract. This roadmap exists only to sequence implementation of that contract.

The roadmap deliberately has **six milestones**. Detailed work is decomposed into small issues/PRs within those milestones.

The migration rules are:

1. preserve working behavior before cleanup;
2. create the canonical authority before deleting duplicate implementations;
3. resolve the P0 native-runtime model before converging other runtime consumers onto it;
4. keep automatic PR CI intentionally small;
5. keep strong verification explicitly available, including to agents with no development environment;
6. preserve the three-axis Docker image architecture;
7. never rebuild or republish Vespa for ordinary Search Kernel source changes;
8. make Cargo/npm/pip/RubyGems the end-user installation surfaces;
9. tie review/release evidence to exact commits and actual distributable artifacts;
10. keep oracle authoring separate from normal verification;
11. delete compatibility paths only after replacement proof.

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

chosen native runtime model
              |
              +--> standalone searchkerneld artifact
              +--> Python managed-local package
              +--> TypeScript/Node managed-local package
              +--> Ruby managed-local package
              +--> Rust Cargo distribution mechanics

User installation surfaces:

    cargo add searchkernel
    pip install searchkernel
    npm install @searchkernel/client
    gem install searchkernel
```

---

## 3. Milestone overview

```text
M1  Canonical build foundation + P0 native-runtime decision
    commands + target registry + image authorities + cache/isolation
    + measured static/shared decision
                      |
                      v
M2  Verification access
    thin PR CI + exact-SHA remote-only GitHub verification
                      |
                      v
M3  Verifier/oracle convergence
    normal build graph + chosen canonical runtime + explicit oracle + simple Universe
                      |
                      v
M4  Ecosystem-native product distribution
    Cargo/npm/pip/RubyGems packaging on the chosen runtime model
                      |
                      v
M5  Release qualification + bootstrap/developer UX
    release-check + support matrix + doctor/docs
                      |
                      v
M6  Cleanup + adversarial final qualification
    delete duplicate authorities only after proof
```

M1 is the foundation. M2 and verifier source-migration preparation may proceed in parallel after the relevant M1 authorities exist. M3 must converge verifier runtime ownership onto the **M1-decided** native runtime model. M4 packages that model for end users. M5 consumes all prior outputs. M6 is deliberately last.

---

# Milestones

## M1 — Canonical build foundation and P0 native-runtime decision

### Goal

Establish every foundational authority that later milestones depend on:

- one public command vocabulary;
- one test/verifier target registry;
- one canonical development environment;
- the preserved three-axis Docker image model;
- explicit cache/worktree ownership;
- a **decided and proven native-runtime distribution model**.

### 1. Inventory before refactoring

Create a checked-in inventory under:

```text
implementation-plan/audit/build-system/
```

covering:

- public `./dev` commands;
- direct Cargo/compiler/interpreter invocations;
- Docker launchers;
- image publishers/consumers;
- native SDK builders/resolvers;
- runtime staging and loader behavior;
- standalone verifier workspaces;
- active C/C++ verifier compilation;
- direct oracle compilation/execution;
- GitHub Actions workflows;
- release/package builders;
- support-matrix rows;
- package-manager artifacts;
- external-consumer/package tests.

Classify each item as:

```text
CANONICAL AUTHORITY
CALLER
TEMPORARY COMPATIBILITY
ORACLE AUTHORING
DEAD / SUSPECTED DEAD
```

Record explicitly:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

and:

```text
55 active current-implementation compiled sources
14 direct oracle/golden-authoring sources
1 uncertain compile fixture
```

### 2. Canonical public command/target authority

Introduce one repository-owned target registry consumed by local and future remote callers.

It drives:

```bash
./dev test --list
./dev verify --list
./dev test <scope>
./dev verify <lane>
```

Preserve/normalize the complete public command surface required by the design:

```bash
./dev doctor
./dev build
./dev ci

./dev test --list
./dev test [scope]
./dev test --remote <scope>
./dev lint [scope]
./dev verify --list
./dev verify <lane>
./dev verify --remote <lane>

./dev universe
./dev release-check
```

M1 only needs to establish the canonical ownership/shape for commands implemented in later milestones; it must not accidentally drop `lint`, `universe`, or `release-check` from the public vocabulary.

Definitions established here:

- `build` = canonical supported product build in the canonical environment;
- `ci` = intentionally cheap mandatory PR gate;
- `test --list` / `verify --list` = finite supported vocabulary.

The target registry must be readable through GitHub so remote-only agents can discover valid operations without running `./dev`.

Add lightweight change-area guidance mapping common repository areas to focused checks.

### 3. Preserve and test the three-axis Docker architecture

Explicit authorities:

#### Toolchain image

```text
searchkernel-ci-toolchain:<toolchain-id>
```

#### Vespa/native image

```text
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
```

#### Thin Search Kernel leaf

```text
searchkernel-ci:<searchkernel-revision>
```

Preserve:

```bash
./dev image toolchain publish
./dev image vespa publish
./dev image publish
```

Hard invariants:

- ordinary Search Kernel source changes do not rebuild/re-upload Vespa;
- the leaf records exact immutable toolchain/Vespa identities;
- execution resolves the exact recorded Vespa identity;
- Vespa is materialized read-only from immutable digest-addressed storage;
- local and CI canonical verification consume the same immutable identities;
- no automatic fallback silently switches to a host toolchain/environment.

Add explicit fail-closed tests:

```text
leaf requires Vespa digest X
host/cache contains X and Y
=> use X

leaf requires X
only Y is available
=> FAIL, never silently use Y

canonical Docker environment unavailable
=> FAIL
never silently run host Cargo and report green
```

### 4. Cache/worktree ownership and performance baseline

Implement/confirm:

- immutable toolchain/Vespa inputs shared by immutable identity;
- sccache shared only as non-authoritative content cache;
- writable Cargo/native incremental outputs persistent per worktree + compatible environment identity;
- unrelated worktrees/agents never share a writable target tree;
- CI writable build state remains job-private.

Before changing cache topology, record baseline timings. Re-run the same benchmark set after convergence:

- unchanged warm `cargo check`;
- small Rust edit + rebuild;
- focused unit test;
- focused native verifier;
- second invocation of the same verifier;
- two verifier lanes sharing dependencies;
- complete `./dev ci`;
- two unchanged consecutive `./dev test <scope>` runs for each first-class language scope, with each run starting in a fresh canonical container;
- an explicit `./dev test ruby` warm-container recreation check because Ruby exercises packaged-gem and real managed-local E2E behavior.

For the second unchanged `./dev test ruby` invocation in a newly created canonical container, acceptance requires:

- zero Rust recompilation;
- zero C/C++ or native-adapter recompilation;
- zero native SDK or Vespa reconstruction;
- zero consumer-runtime download;
- zero generated-code regeneration unless inputs changed;
- reuse of the compatible worktree-scoped Cargo/native outputs and immutable caches created by the first invocation.

Ruby tests, gem/package checks, and E2E still execute on the second run; only unnecessary build/setup work is forbidden.

Exit criterion:

> No material regression in the design-defined warm edit/build/test benchmark set, and container recreation does not recreate compatible build state.

If there is a regression, fix cache/mount ownership rather than creating a second correctness path.

### 5. Decide and prove the P0 native-runtime model

This decision moves **before** verifier runtime convergence.

Preserve `searchkernel-ci-vespa`: it remains the development/build input regardless of consumer package layout.

Compare at minimum:

#### A. Fully static per consumer

Native closure linked into each final consumer executable.

#### B. Mostly-static Search Kernel runtime

```text
Search Kernel native runtime artifact
    + adapter
    + Vespa implementation absorbed statically
    + private dependencies static where practical
    + ordinary platform runtime libraries dynamic
```

#### C. Relocatable shared closure

```text
Search Kernel native runtime artifact
+ required Vespa/private shared libraries
```

Measure/document:

- static archive availability;
- constructor/registry/dead-strip behavior;
- `--whole-archive` needs;
- dynamic-only dependencies;
- artifact size;
- link time/memory;
- relocatability;
- licensing/distribution implications;
- Cargo test-binary duplication;
- runtime identity/loader behavior.

The native-topology spike and the consumer-delivery model are separate decisions.

The current static-link spike has established an important constraint for the pinned Vespa artifacts:

- Candidate A can absorb the Search Kernel adapter/Searchcore layer into a final executable, but the executable still has the large Vespa/private DSO dependency graph;
- Candidate B can produce a more-static `libsearchkernel_vespa.so.2`, but the major Vespa/private DSOs remain because the current build does not expose corresponding usable static archives;
- therefore linker rearrangement alone does not materially eliminate the native runtime closure with today's artifacts.

This does **not** prove Vespa source cannot later be built into a more monolithic artifact. A source-built monolithic/PIC-static Vespa experiment remains a valid optimization, but it is no longer a v1 distribution prerequisite if the shared-closure runtime is acceptable under the lazy cached delivery model.

The chosen runtime topology must pass an external-consumer proof with:

- no ambient `LD_LIBRARY_PATH`;
- no manual Vespa install;
- no repository Cargo config;
- no developer SDK path;
- no cross-worktree/runtime-generation leakage;
- preserved native SDK/Vespa provenance.

Independently, the **v1 consumer delivery architecture** is:

```text
small ecosystem package
        |
        +-- exact runtime identity/digest
        |
first managed-local use/build
        |
        +-- cache hit -> reuse
        |
        +-- cache miss
                -> fetch immutable platform consumer runtime
                -> verify checksum/provenance
                -> atomic install into shared cache
                -> reuse thereafter
```

The consumer runtime is distinct from the developer native-base artifact. It contains only product runtime material (for example `searchkerneld`, `libsearchkernel_vespa.so.2`, required Vespa/private DSOs, and manifest/provenance), not developer-only headers/configured build metadata/static-link inputs.

Initial hosting may use GitHub Release assets and should reuse the proven native-base concepts for immutable generation selection, checksums, and machine caching without conflating the two artifact types.

Before the ADR is final, run **minimal package-manager feasibility prototypes using this lazy-cache delivery mechanism**:

```text
Cargo       publishable/package candidate
            -> first build/use resolves exact runtime as required by native linking
            -> clean external consumer builds/runs
            -> second build/use reuses cache

pip         small wheel
            -> pip install does not need to embed full runtime
            -> first managed-local use downloads/verifies/runs
            -> second use succeeds offline from cache

npm         small client package
            -> npm install does not need to embed full runtime
            -> first managed-local use downloads/verifies/runs
            -> second use succeeds offline from cache

RubyGems    small gem
            -> gem install does not need to embed full runtime
            -> first managed-local use downloads/verifies/runs
            -> second use succeeds offline from cache
```

The Cargo prototype must determine the earliest toolchain-safe acquisition point. If direct native linking requires the runtime at build time, first build may be the acquisition event; do not pretend Rust can defer until process startup unless the native boundary is deliberately changed to support that.

These prototypes need not be production release packaging, but they must expose:

- compressed runtime transfer size;
- platform selection;
- trusted artifact origin;
- exact runtime identity/digest;
- cold-cache first use;
- warm-cache offline reuse;
- concurrent first-use behavior;
- atomic installation;
- prefetch/prewarm behavior;
- package-registry constraints;
- integrity/fail-closed behavior.

If any promised ecosystem cannot support the model cleanly, the runtime delivery ADR remains open and M3 runtime convergence is blocked.

Create a short ADR recording:

```text
chosen native topology for v1
rejected/deferred native topology options
static-link spike evidence
consumer runtime artifact shape
lazy cache/download contract
cold/warm cache proof
Cargo acquisition point
provenance rules
platform assumptions
compressed transfer measurements
```

The permanent invariant is **consumer correctness and stable runtime identity**, not today's `libsearchkernel_vespa.so.2` layout or whether the implementation is one DSO versus a relocatable closure.

Do not remove current #745 consumer-safe behavior until the chosen replacement is proven.

### M1 acceptance

- inventory identifies current authorities/callers;
- canonical target registry exists and is GitHub-readable;
- public command surface including `lint` is preserved;
- image axes remain independently versioned/published;
- image identity resolution is fail-closed;
- no host fallback can silently produce a green canonical result;
- cache/worktree ownership is explicit and benchmarked;
- warm performance shows no material regression;
- P0 native runtime ADR exists with measured evidence;
- chosen native runtime passes external-consumer identity/relocatability proof;
- minimal Cargo/pip/npm/RubyGems feasibility prototypes prove that same runtime model can actually sit behind every promised package-manager UX;
- no working compatibility path has been deleted prematurely.

---

## M2 — Thin CI and first-class remote verification

### Goal

Give local-capable and remote-only agents reliable verification without restoring the historical heavy mandatory PR matrix.

### 1. Automatic PR CI delegates to `./dev ci`

The mandatory PR gate stays intentionally small:

```text
cargo fmt --check
cargo check
cargo clippy -D warnings
```

with exact semantics owned by `./dev ci`.

GitHub Actions becomes conceptually:

```text
checkout exact PR revision
./dev ci
```

Workflow YAML must not independently own Cargo flags, compiler selection, verifier semantics, or SDK/runtime setup.

Contract tests validate public `./dev ci` behavior and minimal workflow delegation, not obsolete workflow names/YAML structure.

### 2. One generic bounded on-demand workflow

This is a first-class interface for agents with GitHub access but no usable development environment.

Inputs:

```text
revision   exact pushed SHA
operation  ci | test | verify
target     known scope/lane when applicable
```

The workflow:

1. validates caller/trust policy;
2. validates operation/target against the M1 registry;
3. checks out the exact requested SHA;
4. proves `git rev-parse HEAD == requested SHA`;
5. executes exactly one canonical command:
   - `./dev ci`;
   - `./dev test <scope>`;
   - `./dev verify <lane>`;
6. publishes structured evidence.

### 3. Trusted control plane

The workflow definition/control plane is loaded from trusted/default-branch infrastructure.

The requested revision is checked out separately as the code under test.

The requested PR/commit must not be able to redefine:

- workflow permissions;
- runner selection/policy;
- secret exposure;
- privileged orchestration used to execute itself.

### 4. Structured evidence

Every run exposes:

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

### 5. Local convenience wrappers

Implement:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

The wrappers:

- resolve exact HEAD;
- reject relevant dirty state by default;
- prove HEAD is pushed;
- dispatch the same workflow;
- capture/wait for the exact run;
- surface failure logs;
- propagate status.

Remote-only agents may dispatch via GitHub UI/API/CLI/connector without local `./dev`.

### 6. Security/concurrency

Remote verification:

- uses read-only repository permissions by default;
- receives no release/deploy/write secrets;
- defines authorized actors;
- defines fork/untrusted-revision behavior;
- uses job-private writable state;
- exposes no arbitrary-shell input.

Do not cancel evidence for SHA A because SHA B is pushed. Verification identity is:

```text
revision + operation + target
```

### M2 acceptance

- every automatic PR-gate failure reproduces with `./dev ci`;
- strong verifiers remain available outside mandatory PR CI;
- remote-only agents can discover and execute bounded exact-SHA checks;
- local wrappers and direct workflow calls produce equivalent semantic commands/evidence;
- stale/dirty/mutable-branch ambiguity fails closed;
- requested code cannot redefine the remote control plane.

---

## M3 — Converge verifier/oracle build ownership onto the chosen runtime

### Goal

Eliminate the secondary normal build architecture under `reference/vespa` while preserving semantic behavior, reference history, and oracle independence.

All normal verifier runtime convergence in this milestone targets the **M1-decided canonical native runtime model**. Do not converge onto a temporary runtime architecture that M4 will immediately replace.

### 1. Migrate the 55 active current-implementation sources

#### Rust — 49 sources

For each verifier family:

1. identify what it proves;
2. identify owning production crate(s);
3. move compiled code into an appropriate normal Cargo target;
4. preserve input/output semantics;
5. point the semantic runner to the new target;
6. compare against the same frozen expectation;
7. delete the standalone workspace only after parity.

Possible homes:

- owning crate integration tests;
- normal test binaries;
- `searchkernel-testkit`;
- a small shared verifier-support crate only where real shared ownership exists.

Do not create one giant verifier crate.

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

### 2. Converge verifier runtime setup

Once verifier binaries are normal targets, move them onto the M1 canonical runtime authority and remove ownership of:

- private normal SDK construction;
- private adapter construction;
- verifier-specific compiler selection;
- verifier-specific Cargo target universes;
- ordinary `LD_LIBRARY_PATH` setup;
- duplicate normal runtime-closure assembly.

Allowed exceptions:

- sanitizers;
- loader-negative tests;
- intentionally independent oracle execution;
- fixtures where environment isolation itself is under test.

### 3. Preserve the reference corpus

Moving compiled verifier code must **not** erase or normalize away the intended role of `reference/vespa`.

Preserve, where they belong:

- behavioral specification;
- contracts;
- scenarios;
- fixtures;
- expected outputs;
- goldens;
- source traces;
- pinned provenance;
- independent oracle sources;
- oracle-authoring tooling;
- historical M0–M14 reference structure;
- lightweight semantic orchestration where useful.

Do not move entire historical milestone trees merely because one compiled driver moved into the normal build graph.

### 4. Make oracle authoring explicit only

Normal:

```bash
./dev verify ...
./dev universe
```

consumes committed frozen behavior.

Remove automatic/environment-detected live-oracle behavior.

Live oracle/golden generation becomes an explicit maintainer operation recording:

- pinned Vespa revision;
- oracle source;
- generation command;
- expected-output diff;
- provenance.

The 14 oracle sources remain independent.

### 5. Universe becomes orchestration only

`./dev universe`:

- enumerates canonical checks;
- invokes them;
- records duration/result;
- continues after failures;
- summarizes;
- prints exact repro commands.

It does not choose another compiler, SDK, adapter, runtime, or verifier build graph.

### M3 acceptance

- all active current-implementation verifier programs compile through normal Rust/native authorities;
- every active verifier has `./dev verify <lane>`;
- semantic behavior/goldens remain unchanged;
- verifier runners use the M1-decided canonical runtime rather than private normal SDK/runtime worlds;
- `reference/vespa` retains its specification/reference/provenance role;
- normal verification never automatically enters live-oracle mode;
- Universe invokes canonical commands only;
- every Universe failure has an exact focused repro.

---

## M4 — Deliver ecosystem-native product distribution

### Goal

Make these end-user contracts true on qualified platforms:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

No supported managed-local user separately installs Vespa, `searchkerneld`, dev images, or repository-specific SDK/runtime setup.

M4 **implements product packaging around the native-runtime model already decided in M1**. It does not reopen that decision unless implementation evidence falsifies the M1 ADR, in which case the ADR/design must be amended explicitly.

### 1. Build and publish one qualified consumer runtime per platform

For each supported platform/runtime identity, build one **consumer** payload whose internal shape follows the M1 native-topology decision:

```text
searchkernel-runtime-<identity>-<platform>
    manifest.json
    bin/searchkerneld            # where managed-local daemon mode requires it
    lib/libsearchkernel_vespa.so.2
    lib/<required Vespa/private runtime closure>
```

The current `libsearchkernel_vespa.so.2 + shared closure` layout is acceptable for v1 if it passes qualification. A future monolithic/static optimization may change the payload internals without changing the package/cache contract.

The consumer runtime must be separate from the developer/build native base:

```text
native base       -> build/developer input
consumer runtime  -> end-user execution artifact
```

Do not ship headers, CMake/configured build metadata, or adapter static-link build inputs merely because the native-base artifact contains them.

Publish the exact qualified consumer runtime once to an immutable artifact origin (initially GitHub Releases is acceptable), with checksum and provenance. Measure both expanded and compressed transfer size.

### 2. Implement the shared lazy runtime cache contract

The canonical managed-local runtime path is:

```text
installed ecosystem package
        |
        v
embedded exact runtime identity
        |
        v
runtime resolver
        |
        +-- valid cache entry -> launch/use
        |
        +-- missing entry
                -> acquire lock
                -> download to temporary path
                -> verify digest/provenance
                -> extract/stage
                -> validate ABI/runtime identity
                -> atomic rename into immutable cache entry
                -> launch/use
```

Define one cross-language contract for:

- default cache root and explicit cache-root override;
- immutable runtime directory naming/addressing;
- manifest schema;
- platform selection;
- artifact URL/origin resolution;
- expected digest;
- download locking;
- temporary/incomplete entry cleanup;
- atomic cache publication;
- cache-hit validation;
- preprovisioned/offline runtime override;
- explicit prefetch/prewarm;
- error taxonomy.

A valid warm-cache hit must perform no network access.

The preferred operational UX should include an explicit prefetch surface (exact spelling may be chosen during implementation), conceptually:

```text
searchkernel runtime fetch
```

so Docker images, CI jobs, and offline deployments can materialize the exact runtime before application startup.

The existing native-base release/fetch/cache implementation should be reused as design precedent or factored where sensible, but consumer runtime code must not depend on developer-only native-base contents.

### 3. Package/runtime identity contract

Before ecosystem-specific packaging is considered complete, define one exact runtime identity per release/platform containing at minimum:

```text
Search Kernel version
runtime/package generation
platform/native target
native ABI version
native runtime artifact digest
pinned Vespa/native identity
```

The resolver must never ask for "latest". Every ecosystem package is published with one exact expected runtime identity/digest.

Implementation requirements:

- Python, TypeScript/Node, and Ruby packages remain small and normally resolve the common consumer runtime lazily instead of embedding independent copies of the full closure;
- Cargo-provisioned native assets are identified by immutable artifact ID/checksum embedded in the publishable crate and acquired at the earliest point required by native linking;
- build/launch-time validation fails closed on ABI/runtime-identity mismatch;
- a preprovisioned runtime override is accepted only if it matches the exact expected identity;
- no package may silently fall back to a different cached/system/worktree runtime generation;
- cross-language conformance tests prove that Python, Node, and Ruby derive the same cache key/runtime identity for the same release manifest.

### 4. Python / pip

Target:

```bash
pip install searchkernel
```

Implement/qualify:

- supported small platform/universal wheel as appropriate;
- exact runtime identity/resolver metadata;
- managed-local default;
- first-use download into the shared cache;
- warm-cache offline reuse;
- optional connect mode;
- clean venv install/execution;
- no duplicate embedded copy of the full native closure unless a platform-specific exception is explicitly justified.

### 5. TypeScript/Node / npm

Target:

```bash
npm install @searchkernel/client
```

The client package carries the exact consumer-runtime identity and resolves the common platform runtime through the shared lazy cache. A registry-hosted platform package remains an allowed fallback if npm constraints make the common artifact origin impractical, but it must represent the same exact qualified runtime identity and must not float independently.

The user installs only the client package.

Qualify from a clean npm consumer project with both cold-cache first use and warm-cache offline reuse.

### 6. Ruby / RubyGems

Target:

```bash
gem install searchkernel
```

Implement/qualify:

- supported gem;
- exact runtime identity/resolver metadata;
- first-use download into the shared cache;
- warm-cache offline reuse;
- managed-local lifecycle;
- optional connect mode;
- clean external `GEM_HOME`;
- no duplicate embedded copy of the full native closure unless explicitly required by RubyGems/platform constraints.

Ruby is not architecturally deferred merely because packaging currently lags.

### 7. Rust / Cargo

Target:

```bash
cargo add searchkernel
cargo build
```

This contract means the publishable `searchkernel` crate enables the native/embedded product by default. M4 must deliberately change the crate feature contract so plain `cargo add searchkernel` is sufficient for local Search Kernel. Remote/connect-only Rust remains an explicit non-default mode/feature for callers that opt out of the native default.

Cargo transparently obtains/links/provisions the chosen M1 runtime model using the same exact consumer-runtime identity/cache contract. The M1 feasibility prototype decides whether acquisition occurs at first Cargo build/link or can be deferred until process execution; native linker requirements are authoritative.

The user must not:

- install Vespa;
- manually download the Search Kernel SDK;
- set `SK_NATIVE_SDK`;
- set `LD_LIBRARY_PATH`;
- use a repository path dependency.

Qualification uses a fresh external Cargo project against the actual publishable/distributable package.

### 8. Reconcile `docs/release-support.json`

Update the machine-readable support matrix only after each artifact path is executable.

Intended first-party architecture:

```text
Rust            embedded native via Cargo
Python          managed-local + optional connect
TypeScript/Node managed-local + optional connect
Ruby            managed-local + optional connect
```

Unsupported platforms/surfaces remain explicit.

### M4 acceptance

- all four ecosystem-native installation commands work on qualified platforms;
- managed-local packages execute without separate Vespa/searchkerneld installation;
- Python/TypeScript/Ruby resolve the same qualified platform consumer runtime payload by exact immutable identity;
- ecosystem packages do not independently embed/rebuild duplicate copies of the full Vespa/native closure;
- cold-cache first use downloads/verifies/atomically installs the exact runtime and executes successfully;
- warm-cache use succeeds with network access unavailable and performs no download;
- concurrent cold starts do not corrupt or duplicate the final cache entry;
- explicit prefetch/prewarm materializes the exact same runtime for Docker/CI/offline deployment;
- every ecosystem package is coupled to an exact runtime identity/digest/ABI and fails closed on mismatch;
- Rust uses the actual publishable/distributable Cargo package and plain `cargo add searchkernel` selects native/embedded behavior by default;
- end-user packages do not depend on development/CI Docker images or developer native-base contents;
- package/runtime identity and provenance remain tied to the M1-decided model and immutable build inputs.

---

## M5 — Make release qualification and developer UX canonical

### Goal

Turn the converged build/distribution system into one coherent release and contributor experience.

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
4. builds the qualified consumer runtime payload;
5. records its compressed/expanded size, exact identity, checksum, and provenance;
6. builds every supported small ecosystem/product artifact pointing at that exact runtime identity;
7. writes release manifest/checksums/provenance;
8. publishes or stages the exact consumer-runtime candidate at its immutable test origin;
9. installs/extracts every ecosystem artifact into clean external environments;
10. executes cold-cache qualification for every lazy managed-local surface;
11. disables network access and executes warm-cache qualification for those same surfaces;
12. executes every supported row in `docs/release-support.json`;
13. runs required ABI/persistence/generated-artifact/provenance checks;
14. fails closed if a supported row lacks executable evidence;
15. freezes the qualified artifact set by immutable content identity/checksum so publication consumes those exact candidates;
16. verifies the published artifacts are byte-identical to the qualified candidates, or verifies canonical package-content identity where an ecosystem necessarily rewrites only an archive/transport envelope.

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
fresh venv + empty runtime cache
pip install candidate
first managed-local operation -> fetch/verify/cache exact runtime
disable network
second managed-local operation -> reuse exact cached runtime
```

#### TypeScript/Node

```text
fresh project + empty runtime cache
npm install candidate
first managed-local operation -> fetch/verify/cache exact runtime
disable network
second managed-local operation -> reuse exact cached runtime
```

#### Ruby

```text
fresh GEM_HOME + empty runtime cache
gem install candidate
first managed-local operation -> fetch/verify/cache exact runtime
disable network
second managed-local operation -> reuse exact cached runtime
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

Release contract tests validate public commands/artifacts/provenance, not historical workflow YAML filenames.

### 4. Publication promotes qualified artifacts; it does not rebuild

The release flow is:

```text
build once
    -> checksum/content identity
    -> clean-install/extract qualification
    -> publish/promote that same candidate
```

Publishing must not rerun the product/native build and substitute a new artifact after qualification.

For registries/tooling that can upload the prepared package directly, publish that exact file. If an ecosystem necessarily regenerates only an outer archive/envelope, retrieve or inspect the published artifact and prove canonical package contents plus runtime identity match the qualified manifest before the release is considered complete.

The release manifest must therefore identify both:

- the qualified candidate artifact/content identity;
- the published registry artifact/content identity.

Any mismatch fails the release.

### 5. Simplify bootstrap

Normal supported host contract:

- Git;
- Docker/container runtime;
- GitHub/repository authentication;
- normal shell/filesystem/worktree support.

The canonical image owns Rust/C++/Python/Node/PHP/Ruby/FlatBuffers and normal build dependencies.

Converge `./dev doctor` toward validating/repairing the host boundary and ability to enter the canonical environment.

Host-native toolchain setup becomes advanced/optional.

Bootstrap acceptance must explicitly prove:

- repeated `./dev doctor` is idempotent enough not to destroy/rebuild compatible healthy state;
- environment/bootstrap failures are distinguished from source/build/test failures;
- no hidden language/native host tooling is required for ordinary correctness.

### 6. Documentation and agent guidance

Rewrite `BUILDING.md` normal path to approximately:

```bash
./dev doctor
./dev build
./dev ci
./dev test --list
./dev test <scope>
./dev lint <scope>
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

- `./dev release-check` is sufficient evidence for one exact revision's advertised artifacts;
- every supported support-matrix row has executable artifact evidence;
- release provenance ties shipped artifacts to exact qualified inputs;
- publication promotes the exact qualified artifact set and cannot silently rebuild/substitute product/runtime bytes after qualification;
- normal bootstrap requires only the supported host boundary;
- repeated doctor/bootstrap is safe/idempotent;
- bootstrap/environment failures are distinct from code failures;
- normal contributor workflow fits in roughly one screen of documentation.

---

## M6 — Delete duplicate authorities and perform final adversarial qualification

### Goal

Remove compatibility machinery only after the converged system is proven, then test every persona and every design requirement.

### 1. Delete obsolete paths

Candidates include:

- standalone active verifier Cargo workspaces;
- duplicate verifier lockfiles;
- verifier-specific normal SDK/runtime setup;
- ad-hoc current-implementation C/C++ compilation;
- duplicated target registries;
- obsolete resolver paths;
- stale workflow-internal contract tests;
- stale release-lane YAML coupling;
- compatibility launchers superseded by the chosen runtime model;
- old normal host-toolchain bootstrap behavior.

For every deletion answer:

> **What canonical component now owns this responsibility?**

If the answer is unclear, do not delete it.

### 2. Explicitly protect

Do not delete or flatten:

- direct oracle sources;
- behavioral specifications/contracts/scenarios;
- committed goldens/source traces/provenance;
- historical M0–M14 reference structure;
- toolchain/Vespa/thin-leaf image split;
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
- worktree isolation proven;
- design-defined performance benchmarks remain acceptable.

#### Remote-only coding agent

- discover target from GitHub-visible state;
- dispatch exact-SHA ci/test/verifier;
- retrieve structured evidence/logs;
- rerun after a new commit.

#### Reviewer

- independently run/dispatch exact-head adversarial verification;
- prove run SHA;
- distinguish PR gate, specialist verification, and release qualification.

#### End user

Clean external consumers prove:

```bash
cargo add searchkernel
pip install searchkernel
npm install @searchkernel/client
gem install searchkernel
```

with managed-local execution and no separate Vespa/searchkerneld install.

#### Release maintainer

```bash
./dev release-check
```

builds and qualifies every advertised surface from actual artifacts.

#### CI / automation

- automatic PR CI is thin;
- remote workflow is bounded;
- trusted control plane enforced;
- runner trust policy enforced;
- no hidden shared mutable state.

#### Oracle maintainer

- normal verification never authors truth;
- explicit oracle generation remains independently available and provenance-preserving.

### 4. Adversarial checks

Try to falsify:

- cross-worktree native artifact contamination;
- stale target/cache reuse;
- fresh canonical-container recreation incorrectly discarding compatible worktree build state;
- unchanged repeated `./dev test ruby` causing Rust/native recompilation, SDK/Vespa reconstruction, generated-code regeneration, or runtime download;
- warm-performance claims;
- stale remote evidence after a new push;
- mutable-branch ambiguity;
- requested revision redefining the remote control plane;
- Vespa identity drift from thin-leaf provenance;
- fallback from canonical Docker execution to host tooling;
- release packages shipping a different runtime than qualification;
- package-manager installs missing runtime assets;
- `LD_LIBRARY_PATH` masking a broken package;
- verifier dependence on private normal SDK/runtime setup;
- loss of historical reference/specification material during verifier migration;
- automatic oracle execution;
- support-matrix claims without executable artifact evidence.

### M6 acceptance

Every Definition-of-Done item in `implementation-plan/build-system.md` is true on the **same exact accepted revision**.

---

## 4. Issue / PR decomposition

The six milestones are roadmap units, not PR units.

### M1 issue families

- build-system inventory;
- target registry and public command authority, including `lint`;
- image-provenance/fail-closed tests;
- cache/worktree ownership;
- performance baseline + post-change benchmark;
- native static/shared runtime spike;
- Cargo/pip/npm/RubyGems runtime-delivery feasibility prototypes;
- runtime ADR + external-consumer proof.

### M2 issue families

- CI delegates to `./dev ci`;
- generic on-demand workflow;
- trusted-default-branch control-plane implementation;
- structured result artifact;
- `./dev --remote` wrappers;
- security/trust regression tests;
- remote-agent documentation.

### M3 issue families

- Rust verifier migrations by family;
- C/C++ current-implementation probe migration;
- convergence onto M1 runtime authority;
- removal of verifier SDK duplication;
- reference-corpus preservation checks;
- explicit oracle authoring;
- Universe orchestration/repro.

### M4 issue families

- consumer runtime artifact + manifest, separated from developer native base;
- immutable consumer-runtime GitHub Release publication/download path;
- shared lazy runtime cache contract and conformance fixtures;
- locking + atomic install + interrupted-download recovery;
- exact package/runtime identity manifest and fail-closed validation;
- runtime prefetch/prewarm + preprovisioned/offline override;
- compressed/expanded runtime size measurement;
- Python pip distribution/resolver;
- TypeScript npm distribution/resolver;
- Ruby gem distribution/resolver;
- Rust Cargo acquisition/linking integration;
- cold-cache + warm-cache cross-language E2E;
- support-matrix reconciliation.

### M5 issue families

- release manifest/provenance;
- package build orchestration;
- clean-install E2E per ecosystem;
- qualify-once/publish-same-artifact promotion path;
- published-artifact identity verification;
- `release-check`;
- doctor simplification/idempotence/error-domain tests;
- BUILDING.md / AGENTS.md convergence.

### M6 issue families

- deletion PRs grouped by removed authority;
- final persona/DoD acceptance audit;
- separate remediation PRs for defects found by that audit.

---

## 5. Hard merge gates

### Gate A — before verifier runtime convergence/deletion

Must have:

- M1 native-runtime ADR;
- chosen runtime model proven externally;
- minimal Cargo/pip/npm/RubyGems feasibility prototypes for that exact runtime model;
- canonical replacement target;
- semantic runner using it;
- unchanged frozen behavior;
- exact verifier passing.

### Gate B — before deleting native-runtime compatibility behavior

Must have:

- M1 chosen runtime model implemented/proven sufficiently for consumers;
- external consumer passes without repository/native environment;
- fail-closed runtime identity;
- no regression in required package/runtime provenance.

### Gate C — before declaring an ecosystem supported

Must have:

- normal package-manager install from the actual distributable ecosystem artifact;
- exact package↔consumer-runtime identity/digest/ABI coupling;
- cold-cache acquisition + checksum/provenance verification + atomic cache install;
- warm-cache offline reuse with no network access;
- fail-closed behavior for runtime mismatch or corrupted cache/download;
- no dependency on developer native-base contents;
- explicit runtime prefetch/prewarm path;
- for Rust, plain `cargo add searchkernel` selecting native/embedded behavior by default and proving its toolchain-safe acquisition point.

### Gate D — before making `release-check` authoritative

Must have:

- reconciled `docs/release-support.json`;
- executable evidence for every supported row;
- release manifest/provenance;
- publication path that promotes the exact qualified artifact set without rebuilding it;
- published-artifact/content identity verification;
- no stale CI-YAML authority.

### Gate E — before removing normal host-toolchain bootstrap

Docker-backed canonical flow must already support:

- build;
- ci;
- lint;
- test;
- verify;
- remote dispatch;
- release qualification where relevant.

---

## 6. Design → milestone traceability

This table assigns every Definition-of-Done requirement from `build-system.md` to an implementation owner. M6 re-verifies them all; it is not the first owner.

| Design DoD | Owning milestone | Planned proof |
|---:|---|---|
| 1 | M2 | `./dev ci` reproduces PR gate |
| 2 | M2 | GHA PR workflow delegates to `./dev ci` |
| 3 | M1 | canonical Docker environment authority |
| 4 | M1 | compatible output persistence tests |
| 5 | M1 | required warm-performance benchmark set |
| 6 | M3 | 55 active compiled sources migrated |
| 7 | M3 | 14 oracle sources excluded from normal build |
| 8 | M3 | `m-api.rs` migrated or deleted with proof |
| 9 | M3 | standalone verifier workspace removal after parity |
| 10 | M3 | C/C++ probes become normal native targets |
| 11 | M3 | no private normal verifier runtime worlds |
| 12 | M3 | reference-corpus preservation acceptance |
| 13 | M3 | every active lane exposed via `./dev verify` |
| 14 | M3 | Universe calls canonical commands only |
| 15 | M3 | Universe exact repro output |
| 16 | M3 | explicit-only oracle authoring |
| 17 | M1 + M4 | native model proof + Cargo package proof |
| 18 | M1 + M4 | external consumer has no repo/`./dev` dependency |
| 19 | M1 | fail-closed runtime identity tests |
| 20 | M1 | relocatability proof in runtime ADR |
| 21 | M1 | static/shared investigation recorded |
| 22 | M1 | native SDK/Vespa provenance preserved |
| 23 | M2 | strong verifiers available on demand |
| 24 | M1 + M2 | one GitHub-readable target registry used remotely |
| 25 | M1 | change-area → focused-check documentation |
| 26 | M2 | bounded remote `ci/test/verify` workflow |
| 27 | M2 | local `--remote` wrappers |
| 28 | M2 | exact pushed-SHA enforcement |
| 29 | M2 | dirty local handoff fails closed |
| 30 | M2 | remote invokes canonical `./dev` semantics |
| 31 | M2 | no verifier-specific workflow semantics |
| 32 | M2 | exact-run wait/log/result propagation |
| 33 | M2 | authorized-actor/runner/permission policy |
| 34 | M2 | trusted control plane + separate tested SHA |
| 35 | M2 | no arbitrary remote shell |
| 36 | M2 + M5 | AGENTS.md exact-SHA protocol |
| 37 | M1 | worktree-scoped mutable target state |
| 38 | M5 | minimal host boundary via `doctor` |
| 39 | M1 | canonical `./dev build` meaning |
| 40 | M3 | no automatic live-oracle mode |
| 41 | M4 + M5 | support matrix reconciled and authoritative |
| 42 | M5 | `release-check` builds/qualifies actual artifacts |
| 43 | M4 + M5 | clean external Cargo package execution |
| 44 | M5 | release checks public artifacts/commands, not YAML internals |
| 45 | M4 | Python/TS/Ruby managed-local architecture |
| 46 | M4 | npm/pip/gem/cargo install commands sufficient |
| 47 | M4 + M5 | clean managed-local language-package E2E |
| 48 | M4 + M5 | clean external Cargo build/native execution |
| 49 | M4 | one qualified managed-local runtime payload |
| 50 | M1 | independent toolchain/Vespa/leaf identities |
| 51 | M1 | normal source publish does not republish Vespa |
| 52 | M1 | leaf records exact immutable identities |
| 53 | M1 + M2 | local/remote verification resolves immutable Vespa identity |
| 54 | M5 | release provenance manifest/checksums |
| 55 | M4 + M5 | end-user artifacts independent of dev images |
| 56 | M6 | delete only after replacement proof |
| 57 | M5 | one-screen normal BUILDING.md flow |
| 58 | M1–M6 | final architecture ownership is discoverable |
| 59 | M1 | Cargo/pip/npm/RubyGems feasibility prototypes gate the runtime ADR |
| 60 | M4 | exact package/runtime identity, digest, ABI, and fail-closed validation |
| 61 | M4 | plain Cargo dependency selects native/embedded default product |
| 62 | M5 | publish/promote exact qualified artifact set; verify published identity |
| 63 | M4 | small packages resolve one immutable consumer runtime through shared cache |
| 64 | M4 + M5 | cold-cache download/verify/atomic-install qualification |
| 65 | M4 + M5 | warm-cache offline reuse with no download |
| 66 | M4 | consumer runtime artifact excludes developer-only native-base surface |
| 67 | M4 + M5 | explicit prefetch/preprovision path with exact identity validation |

---

## 7. Explicit non-goals

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

---

## 8. Final completion checklist

The roadmap is complete only when all are true:

- `./dev` is the local semantic authority;
- the public command surface, including `lint`, is intact;
- GitHub on-demand verification is first-class for remote-only agents;
- automatic PR CI delegates to `./dev ci`;
- toolchain image, Vespa/native image, and thin leaf remain independent;
- ordinary source changes never republish Vespa;
- image identity resolution is fail-closed with no silent host fallback;
- worktrees retain useful warm state without shared mutable-target contamination;
- required warm-performance benchmarks show no material regression;
- the P0 native-runtime model is deliberately chosen and documented before verifier runtime convergence;
- that model has successful minimal Cargo/pip/npm/RubyGems delivery prototypes before M3 treats it as final;
- active verifier code compiles through normal build authorities;
- verifier runners use the chosen canonical runtime rather than private normal SDK/runtime universes;
- `reference/vespa` retains specification/history/provenance material;
- live oracle execution is explicit only;
- Universe is orchestration only;
- Python, TypeScript/Node, and Ruby resolve one qualified consumer runtime payload through the shared lazy cache;
- managed-local ecosystem packages stay small and do not independently embed/rebuild the full native closure;
- cold-cache first use downloads, verifies, and atomically installs the exact runtime;
- warm-cache use succeeds offline without redownloading;
- Docker/CI/offline environments can explicitly prefetch/preprovision the exact runtime;
- consumer runtime artifacts are distinct from developer native-base artifacts;
- every ecosystem package is pinned to an exact fail-closed runtime identity/digest/ABI;
- Rust obtains the chosen runtime transparently through Cargo and native/embedded behavior is the default Cargo product;
- `cargo add searchkernel` works;
- `pip install searchkernel` works;
- `npm install @searchkernel/client` works;
- `gem install searchkernel` works;
- supported managed-local users do not separately install Vespa or `searchkerneld`;
- `./dev release-check` qualifies actual artifacts;
- publication promotes the exact qualified artifact set rather than rebuilding it;
- published artifact/content identity is verified against the qualification manifest;
- release provenance ties artifacts to exact Search Kernel/toolchain/Vespa identities;
- bootstrap is minimal, idempotent, and distinguishes environment failures from code failures;
- duplicate authorities are deleted only after replacement proof;
- every design DoD row passes on one exact final revision.
