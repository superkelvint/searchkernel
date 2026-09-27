# Search Kernel Build-System Convergence Plan

Status: proposed  
Scope: build, test, verifier, native-runtime, and CI convergence  
Baseline audited: `main` at `19ddb707eb2e966e49daab09d011252bb5bfef1c`

## 1. Executive summary

Search Kernel does **not** need a new build system.

It already has the core pieces it needs: Cargo, the native Vespa build, a canonical container image, `./dev`, semantic verifiers, frozen reference data, and consumer packaging tests.

The problem is that responsibility for building and verifying the repository has gradually spread across too many places:

- `./dev`;
- GitHub Actions;
- Docker launchers;
- language/toolchain resolver scripts;
- native SDK discovery and construction;
- native runtime-library setup;
- standalone verifier Cargo projects;
- ad-hoc C/C++ verifier compilation;
- `reference/vespa`;
- Universe;
- pre-commit hooks;
- package/consumer verification.

Several layers independently make decisions about the same concerns: which toolchain to use, where Cargo outputs live, which native SDK or adapter is authoritative, how native executables find `libsearchkernel_vespa.so.2`, how verifier programs are compiled, and which environment a verifier requires.

The goal of this project is therefore:

> **Convergence, not replacement.**

We will preserve the proven implementation pieces and progressively put them behind a small number of explicit authorities. Duplicate paths are deleted only after their canonical replacement is proven.

The target mental model is:

```text
Developer / CI
      |
      v
    ./dev
      |
      +---- canonical development environment
      |
      +---- normal build graph
      |       +---- Cargo
      |       +---- native build
      |       +---- active verifier programs
      |
      +---- verifier orchestration
              +---- scenarios
              +---- frozen expected behavior

Consumers
      |
      v
Search Kernel artifact/package
      |
      +---- must contain enough runtime information/artifacts
            to work WITHOUT the Search Kernel development environment
```

That consumer boundary is a **P0 architectural requirement**.

---

## 2. Users and requirements

This build system serves several distinct users. They share infrastructure, but they do **not** have the same goals.

Treating all of them as a generic "developer" is one reason build systems accumulate accidental complexity: bootstrap concerns leak into CI, reviewer concerns turn into mandatory PR matrices, release qualification leaks into the edit/test loop, and repository-only runtime assumptions leak into downstream packages.

Every major build-system mechanism should therefore answer:

> **Which user is this serving, and which requirement does it satisfy?**

If a piece of machinery cannot answer that clearly, it is a candidate for deletion or consolidation.

### 2.1 User summary

| User | Core question | Primary interface |
|---|---|---|
| Bootstrapper: developer or coding agent | "How do I get this repository working?" | `./dev doctor`, `./dev build` |
| Implementer: developer or coding agent | "How do I know my change works?" | `./dev test`, `./dev verify`, remote variants |
| Reviewer: developer or coding agent | "How do I know this exact proposed change is correct?" | exact-SHA local/remote test and verify |
| End user / downstream application developer | "How do I use Search Kernel without knowing its build internals?" | supported Search Kernel package/dependency |
| Release / distribution maintainer | "Can this exact revision be shipped?" | `./dev release-check`, packaging/publish operations |
| CI / automation | "How do I execute canonical checks deterministically?" | `./dev ci`, exact-SHA dispatched `./dev` operations |
| Reference / oracle maintainer | "How do I deliberately regenerate behavioral truth?" | explicit oracle/golden-authoring workflow |

These personas define the product requirements for the build system.

---

### 2.2 Bootstrapper — developer or coding agent

#### Goal

Go from a fresh clone or fresh development machine to a known-good Search Kernel development environment without repository archaeology.

#### Primary workflow

```bash
git clone ...
cd searchkernel

./dev doctor
./dev build
```

#### Requirements

The bootstrap path must:

- define a deliberately small **host contract**: Git, Docker/container runtime, repository authentication (normally GitHub CLI), and ordinary shell/filesystem support;
- detect missing host prerequisites clearly;
- establish the canonical development environment deterministically;
- keep Rust, C/C++, Python, Node, PHP, Ruby, FlatBuffers, and normal build dependencies owned by the canonical image rather than requiring them on the host;
- provision or locate the pinned native/Vespa inputs through one authority;
- avoid requiring developers to know internal SDK/cache paths;
- avoid manual `LD_LIBRARY_PATH`, compiler, Python, Node, PHP, Ruby, or FlatBuffers setup;
- distinguish repairable bootstrap failures from source/test failures;
- be usable by a coding agent with terminal access;
- be idempotent enough that rerunning it does not destroy useful compatible caches;
- document the normal path in approximately one screenful.

`./dev doctor` owns validation/repair of the supported **host boundary**. Host-native toolchains may remain available as an explicitly advanced workflow, but they are not a prerequisite for ordinary repository correctness.

`./dev build` means the canonical supported Search Kernel product build in the canonical development environment. It must not be an alias for an arbitrary host Cargo invocation.

The bootstrapper should not need to understand:

- historical verifier structure;
- native link topology;
- oracle generation;
- GitHub Actions implementation;
- release packaging internals.

#### Success criterion

A new supported machine can reach a successful ordinary build through the public `./dev` interface without undocumented shell incantations.

---

### 2.3 Implementer — developer or coding agent

#### Goal

Change code and determine quickly whether the change is correct.

This user optimizes for the **edit -> build -> focused test -> focused verifier** loop, not for exhaustive release qualification.

#### Primary workflow

```bash
./dev build
./dev test <scope>
./dev verify <lane>
```

When the local environment cannot execute the relevant check:

```bash
git push
./dev test --remote <scope>
./dev verify --remote <lane>
```

#### Requirements

The implementation loop must:

- reuse warm Cargo/native build state across compatible invocations;
- keep mutable build state isolated per worktree (plus compatible toolchain/environment identity), so multiple agents/worktrees do not share a writable Cargo target tree;
- make focused checks cheap relative to full qualification;
- expose semantic test/verifier names rather than implementation directories;
- provide canonical discovery through `./dev test --list` and `./dev verify --list`;
- maintain a lightweight documented mapping from major change areas to the smallest relevant checks, without inventing a dependency-analysis framework;
- produce deterministic exit status;
- print useful failure output;
- print exact local reproduction commands;
- make the relevant specialist verifiers available even when they are not mandatory PR CI;
- provide remote execution for checks an agent cannot run locally;
- run remote checks against the exact pushed commit SHA;
- fail closed by default if the local working tree/index differs from the commit being requested for remote verification, so the user cannot accidentally verify stale committed code while looking at newer uncommitted changes;
- avoid requiring the implementer to understand GitHub Actions YAML;
- avoid reflexively running Universe for every change.

The implementer must be able to choose verification proportional to the change.

#### Success criterion

A developer or agent can identify and run the smallest relevant correctness checks, locally where possible and remotely where necessary, without inventing environment setup.

---

### 2.4 Reviewer — developer or coding agent

#### Goal

Evaluate someone else's proposed change and answer:

> **What was actually verified for this exact revision, and can I independently challenge it?**

Review is distinct from implementation. A reviewer may not have the author's local environment, may deliberately choose different/adversarial checks, and must not accidentally accept evidence from an earlier commit.

#### Primary workflow

Conceptually:

```bash
checkout exact PR/head revision

./dev test <scope>
./dev verify <lane>

# or, when local execution is unavailable:
./dev test --remote <scope>
./dev verify --remote <lane>
```

#### Requirements

Review tooling must:

- make the exact tested SHA explicit;
- never treat a run against an older commit as evidence for the current PR head;
- support targeted independent/adversarial verification;
- let reviewers rerun the same canonical checks authors run;
- let reviewers use remote infrastructure without learning CI implementation details;
- surface run IDs/URLs and failed logs for remote checks;
- make local reproduction commands obvious;
- avoid mutable branch names as verification identity;
- require remote evidence to correspond to a committed, remotely available SHA and reject ambiguous dirty-worktree handoff by default;
- keep verifier semantics in `./dev`, not in reviewer-specific CI workflows;
- allow a reviewer to distinguish:
  - mandatory PR gate status;
  - explicitly requested specialist verification;
  - full release qualification.

#### Success criterion

Given a PR SHA, a reviewer or reviewing agent can independently run or dispatch relevant checks and can tell unambiguously which exact commit each result covers.

---

### 2.5 End user — downstream application developer

#### Goal

Use Search Kernel in an application without learning the Search Kernel repository's build system.

For this user, the best build-system UX is largely **invisible**.

Conceptually:

```text
add Search Kernel dependency/package
        |
        v
build application
        |
        v
run application
```

#### Requirements

The supported downstream experience must not require:

- `./dev`;
- the Search Kernel repository;
- the Search Kernel CI image;
- a separately installed matching Vespa runtime as the normal product model;
- repository-local Cargo configuration;
- ambient `LD_LIBRARY_PATH`;
- knowledge of Search Kernel worktree or SDK paths.

The downstream contract is defined **per supported release surface**, not by one generic consumer model.

The machine-readable authority is `docs/release-support.json`. Every supported row must state its runtime mode and have artifact-level executable qualification. Deferred/unsupported rows must fail closed as unsupported rather than inheriting an accidental promise from source-code presence.

The package/build layer must provide, as appropriate to each supported surface:

- the correct Search Kernel native implementation;
- the required Vespa/private native closure in the chosen distribution form;
- deterministic runtime identity;
- relocatability according to the supported platform contract;
- clear failure when required native artifacts are unavailable or incompatible;
- a documented platform/support matrix;
- a clean external-consumer qualification path.

Different release surfaces may legitimately have different native-delivery requirements. For example:

- **Rust embedded/native** must prove a clean downstream project built from the **actual distributable Rust artifact** can execute the embedded engine outside the repository with a cleaned runtime environment;
- **searchkerneld** must prove the packaged daemon/runtime tree executes after relocation with repository/native environment variables removed;
- **managed-local language packages** must prove their installed package can launch/use the packaged runtime from its installed location;
- **connect-only clients** do not need to embed Vespa, but must prove that the shipped client artifact can connect to the supported daemon contract.

Source-tree smoke tests are useful but do not substitute for testing the exact artifact that will be shipped.

This persona is why consumer-native runtime correctness is P0.

#### Success criterion

For every row marked supported in `docs/release-support.json`, the actual distributable artifact is built, installed/extracted into a clean external location where applicable, and exercised according to that row's runtime contract with no repository-only workaround.

---

### 2.6 Release / distribution maintainer

#### Goal

Answer:

> **Can this exact revision be shipped, and are its distributable artifacts trustworthy?**

This is deliberately stronger than "does the PR gate pass?"

#### Primary workflow

```bash
./dev release-check
```

plus explicit publishing operations only after qualification succeeds.

`./dev universe` and `./dev release-check` have different meanings:

- `./dev universe` is exhaustive **repository/source qualification**: tests, semantic verifiers, clients, hardening lanes, and historical closures;
- `./dev release-check` is **shipability qualification for one exact revision**. It runs the required source qualification (directly or by consuming exact-SHA evidence), builds the actual release artifacts, then validates every supported row in `docs/release-support.json`.

A green `universe` is therefore necessary evidence for release qualification when the release policy requires it, but it is not by itself proof that the artifacts are shippable.

#### Requirements

Release qualification must include:

- the complete intended release test/verifier set;
- construction of the exact release artifact set;
- artifact-level qualification for every supported row in `docs/release-support.json`;
- external consumer/package qualification, including execution of the actual distributable Rust artifact rather than only repository-path consumption;
- native ABI verification;
- persistence/backward-compatibility checks;
- native SDK and Vespa provenance checks;
- packaging/relocatability checks;
- generated-artifact consistency;
- exact revision identity;
- reproducible failure commands;
- release-artifact provenance.

The release maintainer must be able to distinguish:

```text
PR gate passed
```

from:

```text
release qualified
```

They are not equivalent claims.

Release-only cost must not be imposed on every implementer's edit/test cycle.

#### Success criterion

The maintainer can run one canonical command for an exact revision and obtain evidence that:

1. required source qualification passed for that revision;
2. the exact artifacts intended for publication were built;
3. every supported release surface passed its artifact-level qualification;
4. artifact manifests/checksums/provenance identify that revision and its pinned native dependencies.

Release contract checks must validate these public authorities and artifacts. They must not depend on the internal structure or continued existence of a particular GitHub Actions YAML file.

---

### 2.7 CI / automation

CI is a build-system user even though it is not human.

#### Goal

Execute canonical repository checks deterministically and report their results.

#### Responsibilities

CI has two primary modes:

1. automatic cheap PR gating through `./dev ci`;
2. explicit exact-SHA remote execution requested by a developer or agent.

#### Requirements

CI must:

- be non-interactive;
- operate on an explicit revision;
- use deterministic exit codes;
- invoke canonical `./dev` operations rather than reimplementing them;
- avoid hidden mutable state between jobs;
- isolate writable job state where required;
- reuse immutable/cacheable inputs appropriately;
- produce logs useful to both humans and coding agents;
- expose run identity;
- avoid embedding verifier-specific semantic knowledge in workflow YAML;
- avoid becoming a generic remote shell;
- define an explicit trust policy for executing repository code on self-hosted runners;
- restrict remote-dispatch permissions to authorized repository actors;
- use read-only repository permissions by default and expose no release/write secrets to ordinary remote verification;
- treat a requested revision as potentially untrusted code and ensure the runner/job isolation model is appropriate for that trust level, especially when Docker or other privileged host capabilities are available.

#### Success criterion

GitHub Actions is an execution substrate for Search Kernel commands, not a second implementation of the build/test system.

---

### 2.8 Reference / oracle maintainer

#### Goal

Deliberately create or update the behavioral truth against which Search Kernel is verified.

This is a rare, specialized workflow and should remain visibly different from ordinary development verification.

#### Primary workflow

Conceptually:

```text
select exact pinned Vespa revision
        |
        v
build/run independent oracle
        |
        v
inspect/canonicalize result
        |
        v
freeze/update golden
        |
        v
review and commit provenance
```

#### Requirements

Oracle authoring must:

- remain independent from the Search Kernel implementation path;
- require an explicit maintainer action;
- never be entered automatically by normal verification based on environment detection;
- never regenerate goldens as a side effect of ordinary build/test/CI;
- record the exact Vespa/source provenance;
- make changes to expected behavior reviewable;
- permit specialized one-off compilation where necessary;
- prioritize reference integrity over ordinary developer convenience.

The oracle maintainer does **not** need oracle tooling to be part of the normal Cargo build graph.

#### Success criterion

A maintainer can intentionally opt into live-oracle execution and regenerate reference behavior from the pinned upstream implementation, while ordinary developers, agents, reviewers, CI, and Universe consume only reviewed frozen results by default.

---

### 2.9 Cross-persona product requirements

Several requirements follow directly from serving all seven users:

1. **One public vocabulary.** Local developers, agents, reviewers, CI, and release tooling should refer to the same test/verifier names.
2. **One semantic implementation.** Remote execution and CI must call the same `./dev` operations as local execution.
3. **Exact revision evidence.** Review and release claims are always tied to an exact commit.
4. **Cheap path versus strong path.** The normal edit/PR path stays fast; strong specialist and release verification remains available explicitly.
5. **Repository environment is not consumer runtime.** Development convenience must never become a downstream requirement.
6. **Oracle authoring is not verification.** Generating truth and checking against truth remain separate operations.
7. **Warm performance matters.** Hermetic development cannot imply rebuilding everything on every command; mutable warm build state is scoped per worktree and compatible environment, while immutable/content-addressed caches may be shared.
8. **Release promises are machine-readable.** `docs/release-support.json` defines the supported downstream surfaces, and release qualification must execute the actual artifacts for every supported row.
9. **Failures are actionable.** Human or agent users should receive a canonical reproduction command whenever one exists.
10. **No hidden second build system.** GitHub Actions, Universe, verifier directories, release checks, and packaging scripts consume canonical authorities instead of redefining them.
11. **Every mechanism needs a persona.** New build-system machinery should identify which user and requirement justify its existence.

---

## 3. Core architectural invariant

For every build-system concept, exactly one component should be allowed to make the architectural decision. Everyone else consumes that decision.

Target ownership:

| Concern | Authority |
|---|---|
| Public development interface | `./dev` |
| Normal development toolchain | canonical Search Kernel image |
| Entering that environment | one container launcher |
| Rust compilation | root Cargo build graph |
| Native Search Kernel compilation | canonical native build machinery |
| Native base identity | native SDK authority |
| Adapter/runtime relationship | build/package/link layer |
| Active verifier compilation | normal Rust/native build graph |
| Verifier invocation | `./dev verify <lane>` |
| Frozen behavioral evidence | `reference/vespa` |
| Oracle authoring | explicit maintainer path |
| Repository/source qualification | `./dev universe` |
| Release support matrix | `docs/release-support.json` |
| Release/shipability qualification | `./dev release-check` |
| Local mutable build state | per-worktree + compatible-environment target/cache authority |
| PR gate | `./dev ci` |
| GitHub Actions | thin invocation of `./dev` |
| Explicit remote verification | one generic exact-SHA workflow-dispatch adapter that calls `./dev` |

The plan is successful only if it **reduces the number of independent authorities**.

---

## 4. P0 consumer-runtime design constraints

The development environment is not the product runtime.

Search Kernel embeds Vespa. Therefore the native implementation required by Search Kernel must ultimately reach downstream users in some form. A successful development build is not enough; the resulting consumer artifact must work outside this repository.

### P0.1 — Consumer artifacts must be self-sufficient

A supported Search Kernel distribution surface that carries or launches the native engine must not rely on:

- `./dev`;
- the Search Kernel CI image;
- repository-specific shell initialization;
- verifier scripts;
- ambient `LD_LIBRARY_PATH`;
- a globally installed Search Kernel adapter;
- a developer's local SDK directory;

merely to locate the Search Kernel native implementation.

A consumer must be able to build/install Search Kernel through the supported mechanism and run the resulting program without reproducing Search Kernel's repository environment.

### P0.2 — Runtime resolution belongs to the build/package layer

The relationship:

```text
consumer executable
      |
      v
Search Kernel native runtime
      |
      v
Vespa/private native closure
```

must be established by the supported build/package architecture.

Normal callers must not establish that relationship themselves.

This is not an acceptable permanent product design:

```bash
LD_LIBRARY_PATH=/some/searchkernel/path my-program
```

even if `./dev` can hide it from repository developers.

### P0.3 — Exact runtime identity

A consumer artifact must not silently load:

- an adapter from another worktree;
- an adapter from another Cargo target directory;
- an old globally installed adapter;
- a verifier-specific adapter;
- another build generation that happens to expose the same SONAME.

Where a distinct runtime artifact remains, its identity must be tied to the build/package that produced the consumer artifact, and failures should be fail-closed rather than silently selecting an ambient library.

### P0.4 — Relocatability

Supported build/package outputs must not require arbitrary developer-machine absolute paths at runtime.

Where possible, runtime relationships should be output-relative, package-relative, or otherwise relocatable.

### P0.5 — Development and consumer behavior must agree

CI and `./dev` must not pass merely because they inject a runtime environment that downstream users do not receive.

Focused qualification must exercise the **actual distributable artifacts** outside the Search Kernel workspace. Any surface carrying/launching the native engine must execute with `LD_LIBRARY_PATH` and repository-native overrides unset unless that surface's public contract explicitly defines another platform-native mechanism.

Current `main` already has consumer-safe runtime work from PR #745, including an external consumer smoke path. This plan treats that behavior as a required invariant. The current mechanism is not necessarily the final mechanism.

---

## 5. P0 native-distribution decision

Before investing further in increasingly elaborate shared-runtime machinery, Search Kernel should explicitly determine the smallest native distribution model that satisfies the consumer contract.

Because Vespa is embedded, there is no option where the Vespa implementation simply disappears. The bytes must reach the user somehow.

The realistic models are:

### A. Native closure statically linked into every final consumer executable

```text
Rust executable
    + Search Kernel adapter
    + Vespa implementation
    + private static dependencies
```

This has the simplest runtime resolution but may duplicate a very large closure into every Rust binary/test executable and may impose unacceptable link time, memory, or archive-topology requirements.

### B. Mostly-static Search Kernel runtime artifact

```text
Rust executable
      |
      v
libsearchkernel_vespa.so.2
      |
      +-- Search Kernel adapter
      +-- Vespa implementation statically absorbed
      +-- private third-party dependencies statically absorbed where practical
      +-- ordinary platform runtime libraries remain dynamic
```

This is a particularly attractive candidate.

It preserves the clean C ABI boundary and avoids embedding the full Vespa closure into every Cargo test binary while potentially reducing distribution to one Search Kernel-specific runtime artifact.

### C. Relocatable shared native closure

```text
Rust executable
      |
      v
libsearchkernel_vespa.so.2
      |
      +-- Vespa shared libraries
      +-- required private third-party shared libraries
```

If a mostly-static closure is impractical, this remains valid, but the whole closure must be packaged deliberately and runtime resolution must remain consumer-safe.

### D. System-installed Vespa/native dependencies

This is not an acceptable default product model for embedded Search Kernel.

### Required investigation

Determine, with a small concrete build spike:

1. whether the Vespa libraries required by the Search Kernel adapter are available or can reasonably be produced as static archives;
2. whether static constructors, registries, factory registration, or dead stripping require `--whole-archive` or equivalent treatment;
3. which private third-party dependencies can be statically absorbed;
4. which dependencies must remain dynamic;
5. the size of the resulting runtime artifact;
6. link-time and link-memory impact;
7. licensing/distribution implications of the actual native closure;
8. whether model B can produce one relocatable `libsearchkernel_vespa.so.2` plus ordinary platform libraries.

Do **not** remove the current consumer-safe #745 path until a replacement passes the external-consumer qualification.

The permanent invariant is consumer runtime correctness, not the current loader implementation.

---

## 6. Why not replace everything with Nix, Bazel, Buck2, xtask, or just?

These alternatives are worth acknowledging explicitly.

### `./dev`

A stable repository front door is a conventional pattern, similar to "Scripts to Rule Them All". Search Kernel is not inventing a novel abstraction here.

### `cargo xtask`, `just`, and `cargo-make`

These can implement command routing, but replacing `./dev` with one of them does not solve the architectural problems identified here. The desired `./dev` end state is deliberately thin enough that rewriting it in Rust or another task runner would add migration cost without removing a build authority.

### Nix / Bazel / Buck2

These systems can provide stronger hermetic build execution, declared dependencies, sandboxing, provenance, and caching.

They do **not**, by themselves, eliminate the requirement to design Search Kernel's downstream native distribution model for arbitrary Cargo consumers. A non-Nix consumer still needs a supported runtime artifact/package layout.

Adopting one would also turn this project from a convergence effort into a build-system replacement.

That is an explicit non-goal for this phase.

The choice is deliberate:

> Keep Cargo + Docker, converge their ownership boundaries, and solve consumer-native distribution at the product/package layer.

---

## 7. Canonical development environment

Normal build/test/verification should run in the canonical Search Kernel image.

The supported host boundary should remain intentionally small: Git, Docker/container runtime, repository authentication, and ordinary shell/filesystem support. Language runtimes and compilers required for ordinary correctness belong to the canonical image. Host-native Rust/Node/Python/etc. may remain as an advanced convenience path, not as a prerequisite for the normal workflow.

The container owns the normal development toolchain, including the supported versions of:

- Rust/Cargo;
- C/C++;
- Python;
- Node/npm;
- PHP;
- Ruby;
- FlatBuffers;
- native build prerequisites.

Once inside that environment, ordinary invocations of these tools should be ordinary commands.

Do not grow another forest of `resolve-*` scripts merely for symmetry.

Resolver scripts remain appropriate at real boundaries such as:

- host bootstrap;
- `doctor`;
- explicit host-native workflows;
- native SDK lookup;
- image construction.

The canonical container is an **environment boundary**, not a clean-build boundary.

---

## 8. Build/cache performance constraint

The guiding principle is:

> **Hermetic inputs, persistent compatible outputs.**

Moving more execution through the canonical image must not cause every invocation to behave like a clean build.

Normal local development should preserve compatible:

- Cargo incremental state;
- Cargo registry/git caches;
- generated/build-script outputs;
- native build artifacts;
- immutable Vespa/native-base artifacts;
- appropriate language package caches;
- Docker/BuildKit layers.

`CARGO_TARGET_DIR` must have one explicit ownership model for normal builds.

For local canonical-container development, writable Cargo/native incremental state must persist across compatible `./dev` invocations **without being globally shared between unrelated worktrees**. The cache key/namespace must include enough identity to prevent cross-worktree or cross-toolchain contamination. Immutable/content-addressed inputs and compiler caches may be shared safely where their own provenance model permits it.

CI should isolate writable build outputs per job unless a cache is explicitly designed to be non-authoritative and content-addressed. Local development should optimize strongly for the warm edit-build-test loop.

Before declaring convergence complete, benchmark:

- unchanged warm `cargo check`;
- a small Rust edit plus rebuild;
- a focused unit test;
- a focused native verifier;
- the second invocation of the same verifier;
- two verifier lanes sharing dependencies;
- the complete `./dev ci` gate.

If the canonical-container boundary causes a material regression, fix the cache/mount boundary rather than creating a second build path.

---

## 9. Audit: `reference/vespa`

On the audited baseline, `reference/vespa` contains 70 compiled-language source files:

```text
50 Rust
 4 C
12 C++
 4 Java
----
70 total
```

Those files do **not** all have the same role.

### 9.1 Active normal-verification sources: 55

These are compiled to verify the current Search Kernel implementation:

```text
49 Rust
 4 C
 2 C++
----
55 total
```

This is the principal migration set.

It consists of:

- active milestone/API compile-contract Rust sources;
- Rust verifier drivers and support libraries;
- the Rust KStem parity driver;
- four native C probes;
- two SearchReply/current-implementation C++ probes.

These should eventually be owned by normal Cargo/native build targets.

### 9.2 Oracle/golden-authoring sources: 14

These are independent reference implementations:

```text
10 C++
 4 Java
----
14 total
```

C++ oracle sources:

- `reference/vespa/m2/oracle/embedded_crud_spike.cpp`
- `reference/vespa/m4/oracle/m4_text_oracle.cpp`
- `reference/vespa/m8/oracle/m8_local_oracle.cpp`
- `reference/vespa/m9/oracle/m9_attribute_grouping_oracle.cpp`
- `reference/vespa/m9-general-grouping-verifier/oracle/m9_recursive_grouping_oracle.cpp`
- `reference/vespa/m10/oracle/m10_ranking_oracle.cpp`
- `reference/vespa/m12/oracle/m12_tensor_oracle.cpp`
- `reference/vespa/m13/oracle/m13_tensor_oracle.cpp`
- `reference/vespa/m14/oracle/m14_hnsw_oracle.cpp`
- `reference/vespa/searchreply/oracle/searchreply_oracle.cpp`

Java oracle sources:

- `reference/vespa/m3/kstem/oracle/KStemOracle.java`
- `reference/vespa/m3/oracle/src/searchkernel/m3/LinguisticsOracle.java`
- `reference/vespa/m6/oracle/src/searchkernel/m6/SchemaOracle.java`
- `reference/vespa/m7/oracle/src/searchkernel/m7/QueryOracle.java`

These are used to generate or independently validate frozen behavioral truth and should remain conceptually separate from the normal Search Kernel build.

### 9.3 One Rust source requires investigation

`reference/vespa/api-contract/src/bin/m-api.rs` is described as a compile-only stable API fixture, but it was not found in the current normal milestone/product verifier entrypoints inspected during the audit.

Before migrating it:

```text
prove active -> migrate
prove obsolete -> delete
```

Do not preserve dead build machinery merely for symmetry.

### 9.4 Important wording

`reference/vespa` is inside the repository, so the issue is not that it is "outside the source tree".

The precise problem is:

> `reference/vespa` has become both a specification/reference corpus and a secondary mixed-language build tree outside the normal product build graph.

---

## 10. Verifier versus oracle

The architecture should use a simple distinction:

> **Oracle = generates truth.**  
> **Verifier = checks Search Kernel against truth.**

### Oracle authoring

```text
pinned Vespa
     |
     v
independent C++/Java oracle
     |
     v
reviewed output
     |
     v
committed golden
```

This is a rare maintainer operation.

Oracle code may remain under `reference/vespa` because it is part of the provenance for frozen reference behavior.

It does **not** need to participate in:

- normal `cargo build`;
- normal `cargo test`;
- `./dev ci`;
- normal `./dev verify`;
- normal Universe execution.

Where current verifier behavior has an automatic "live oracle" mode, convergence should prefer making oracle execution an explicit maintainer/authoring operation.

### Normal verification

```text
compiled Search Kernel verifier target
        +
committed scenario/golden
        |
        v
      PASS/FAIL
```

Compiled code required for this path belongs to the normal build graph.

---

## 11. Target role of `reference/vespa`

The eventual role should be:

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
- lightweight verifier orchestration.

It should not be a second build system required to test today's Search Kernel.

Normal verification should progressively stop depending on:

- standalone verifier Cargo workspaces;
- verifier-specific Cargo lockfiles;
- direct `cc`/`c++` compilation for current-implementation probes;
- verifier-specific SDK construction;
- verifier-specific runtime-loader setup;
- verifier-owned Cargo/cache universes.

Historical M0-M14 directories may remain. We are not erasing useful specification history.

---

## 12. Public command surface

The normal developer interface should converge toward:

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

Advanced maintainer commands may continue to exist for image publishing, SDK inspection, protocol generation, oracle/golden generation, worktrees, and release maintenance.

`./dev build` is the canonical supported product build in the canonical development environment.

`./dev test --list` and `./dev verify --list` expose the finite public vocabulary used by developers, agents, reviewers, CI, and release qualification. A small documented change-area-to-check mapping should point users toward relevant focused checks; it is guidance, not a second dependency-analysis system.

A developer should not need advanced maintainer commands to understand an ordinary build.

---

## 13. Explicit remote execution for agents

A major reason the heavy verifier suite existed in CI was practical: coding agents often cannot execute native, sanitizer, Vespa, or other specialist checks in their own local environment.

Simplifying mandatory PR CI must **not** remove agents' access to those checks.

The design requirement is:

> **Give agents a remote form of the existing canonical commands.**

Local execution:

```bash
./dev verify vespa.ranking
./dev test rust
```

Remote execution after pushing:

```bash
./dev verify --remote vespa.ranking
./dev test --remote rust
```

The remote form is not a new verifier implementation. It is only an execution-location switch.

### 13.1 Semantic equivalence

This invariant is mandatory:

```text
./dev verify foo
```

and:

```text
./dev verify --remote foo
```

must ultimately execute the same canonical operation:

```text
./dev verify foo
```

Likewise:

```text
./dev test rust
./dev test --remote rust
```

must share the same underlying `./dev test rust` semantics.

The architecture is:

```text
                         +-- local ------------------+
                         |                           v
Agent ---- ./dev verify foo                  canonical environment
                         |                           ^
                         +-- --remote --> GitHub ----+
                                      |
                                      v
                              ./dev verify foo
```

GitHub Actions does not own verifier semantics.

### 13.2 One generic on-demand workflow

Provide one bounded `workflow_dispatch` workflow for explicit remote verification.

Its conceptual inputs are:

```text
revision   exact pushed commit SHA
operation  verify | test
target     known verifier lane or test scope
```

The workflow:

1. checks out the exact requested SHA;
2. verifies that `git rev-parse HEAD` equals that SHA;
3. executes exactly one canonical command:
   - `./dev verify <target>`, or
   - `./dev test <target>`;
4. returns the resulting exit status and logs.

The workflow must **not** know how `vespa.ranking`, persistence, Rust tests, or any other lane works internally.

It must not contain verifier-specific build logic.

### 13.3 Exact pushed SHA is mandatory

Remote verification is evidence about a specific commit, not about a branch name.

If the agent pushed:

```text
7bc93a0...
```

then the dispatched request must identify:

```text
revision = 7bc93a0...
operation = verify
target = vespa.ranking
```

Do not dispatch against a mutable PR branch and infer which revision was tested.

Before dispatch, `./dev` should establish that the requested HEAD is available on the remote. If the current commit has not been pushed, remote verification should fail with a clear instruction rather than silently test an older commit.

The result should identify at minimum:

```text
revision: 7bc93a0...
command:  ./dev verify vespa.ranking
run:      <GitHub Actions run URL/ID>
result:   PASS | FAIL
```

A verification run against an earlier commit must never be presented as verification of a newer PR head.

Remote verification also verifies a **committed state**, not an arbitrary local working-tree snapshot. By default, `--remote` must fail if relevant tracked/index/untracked source changes mean the current working tree differs from the commit being dispatched. The user must first commit and push the state they intend to verify. Any future override must be explicit and must never allow the resulting run to be described as verification of uncommitted local changes.

### 13.4 Remote-execution trust boundary

The on-demand workflow executes repository code, potentially on self-hosted infrastructure with Docker access. Exact-SHA provenance is necessary but not sufficient; the workflow needs an explicit trust model.

The first version must:

- permit dispatch only by authorized repository actors;
- request read-only repository permissions unless a narrower operation requires more;
- expose no release, publishing, deployment, or write-capable secrets to ordinary remote verification;
- treat the requested revision as untrusted executable input;
- use job-private writable state;
- define whether fork/untrusted-PR revisions are rejected or run in a separately hardened execution environment;
- fail closed rather than silently falling back to a more privileged runner/path.

The goal is not to build a general sandboxing platform. It is to ensure that a convenient remote test button does not become a privileged arbitrary-code execution path.

### 13.6 `./dev` owns the remote UX

Agents should not normally need to invoke `gh workflow run` or manually locate Actions runs.

`./dev verify --remote <lane>` and `./dev test --remote <scope>` should:

1. resolve the current exact commit SHA;
2. require that SHA to exist remotely;
3. dispatch the generic on-demand workflow;
4. capture the returned run identity deterministically;
5. wait for that specific run;
6. print the run URL/ID;
7. surface failed-step logs when the run fails;
8. exit success/failure consistently with the remote operation.

A failure should end with a canonical local reproduction command where applicable:

```text
FAIL: vespa.grouping.yql

REPRO:
./dev verify vespa.grouping.yql
```

This gives humans and coding agents the same interface.

### 13.5 Bounded operations only

Remote execution is **not** a generic remote shell.

Supported forms are bounded to canonical repository operations such as:

```bash
./dev verify --remote <known-lane>
./dev test --remote <known-scope>
```

Verifier targets must be validated against the same canonical registry used by:

```bash
./dev verify --list
```

Test scopes should similarly be a finite supported set.

Do not add an interface such as:

```text
./dev remote "<arbitrary shell command>"
```

This preserves a reviewable and safe remote execution surface.

### 13.7 PR labels are not the primary interface

Labels may be added later as a convenience if some automation can manipulate labels but cannot dispatch Actions directly.

They should not be the primary model because label state is poor verification state and makes reruns awkward.

The canonical interface is explicit workflow dispatch hidden behind `./dev`.

### 13.8 Agent protocol

The repository's agent guidance should eventually state:

```text
After pushing changes, run the relevant verification remotely when
the local environment cannot execute it:

    ./dev verify --remote <lane>
    ./dev test --remote <scope>

Remote verification always tests the current pushed HEAD exactly.

Do not claim verification from a run against an earlier commit.

If a remote verifier fails:
1. inspect the returned failure log;
2. reproduce locally when possible using the printed command;
3. fix the issue;
4. push the fix;
5. rerun the remote verifier against the new HEAD.
```

Agents should choose relevant specialist checks rather than reflexively running Universe.

### 13.9 CI separation

The intended division is:

| Mechanism | Trigger | Purpose |
|---|---|---|
| `./dev ci` | automatic on every PR | cheap mandatory gate |
| `./dev verify <lane>` | explicit local | specialist verification |
| `./dev verify --remote <lane>` | explicit remote | specialist verification on CI infrastructure |
| `./dev test --remote <scope>` | explicit remote | tests unavailable locally |
| `./dev universe` | explicit | full repository/source qualification |
| `./dev release-check` | explicit release workflow | build and qualify actual distributable artifacts |

This is deliberately different from rebuilding the old automatic heavy CI matrix.

The architectural requirement is:

> **Strong verifiers remain available locally and through explicit remote execution, even though they are not mandatory PR CI.**

Do not turn remote-run reporting into a second infrastructure project. Exact SHA, canonical command, run identity, result, and failure logs are sufficient for the first version.

---

## 14. Migration plan

### Phase 0 — Freeze and inventory

Inventory every current path that:

- launches Docker;
- chooses a compiler/interpreter;
- invokes Cargo;
- defines standalone Cargo workspaces;
- compiles C/C++;
- builds Java oracle programs;
- chooses/builds a native SDK;
- changes `LD_LIBRARY_PATH`;
- constructs RPATH/SONAME/runtime paths;
- launches native binaries;
- creates verifier-specific caches;
- runs historical verifiers;
- performs CI-only setup;
- performs Universe-only setup.

Classify each as:

```text
CANONICAL AUTHORITY
CALLER
TEMPORARY COMPATIBILITY
ORACLE AUTHORING
DEAD
```

Do not refactor first and inventory later.

Completion criterion: a reviewer can answer which component currently owns every build/runtime decision without repository archaeology.

---

### Phase 1 — Resolve the P0 native-distribution model

Run the mostly-static closure investigation described in section 5.

Preserve the current #745 consumer-safe runtime mechanism while investigating alternatives.

Whatever model wins must prove:

1. a clean downstream Cargo consumer built from the actual distributable Rust artifact builds and executes outside the repository;
2. native-carrying supported artifacts execute with `LD_LIBRARY_PATH` and repository-native overrides unset;
3. they do not depend on repository Cargo configuration;
4. runtime identity cannot silently cross worktrees/build generations;
5. required native payload is relocatable according to its supported distribution contract;
6. every supported native-carrying row in `docs/release-support.json` has artifact-level executable evidence;
7. native SDK/provenance guarantees remain intact.

This phase is allowed to conclude that the current relocatable shared-runtime approach is the correct model. It must not assume that conclusion before testing the mostly-static alternative.

---

### Phase 2 — Make the PR gate a canonical `./dev ci`

Define:

```bash
./dev ci
```

as exactly the intentionally minimal PR gate:

```text
cargo fmt --check
cargo check
cargo clippy -D warnings
```

Do not reintroduce native/full/hardening/package verification into every PR during this convergence effort.

Reduce GitHub Actions conceptually to:

```text
checkout
./dev ci
```

Actions should not independently know Cargo semantics, native runtime setup, compiler paths, or Docker mount policy beyond what is necessary to invoke the repository interface.

Completion criterion: every PR-gate failure has exactly one local reproduction command, `./dev ci`.

Repository contract tests should validate the public `./dev ci` behavior/invariants and only the minimal workflow delegation contract. They must not encode obsolete internal workflow names or implementation details as architectural requirements.

---

### Phase 3 — Make `./dev` a thin router

`./dev` should primarily:

1. parse the requested operation;
2. decide host versus canonical environment;
3. dispatch the existing implementation.

Do not rewrite the entire script.

Move implementation responsibility downward into Cargo/native/test tooling where it belongs.

Keep compatible target/cache state persistent across container invocations using a per-worktree + compatible-environment writable namespace. Do not reintroduce shared mutable Cargo targets across unrelated agents/worktrees.

During this phase, define `./dev build` as the canonical product build and add `./dev test --list` alongside the existing verifier discovery.

Completion criterion: the routing model is obvious from the top of `./dev`, repeated local commands reuse compatible work, and two concurrent task worktrees cannot mutate the same Cargo target tree.

---

### Phase 4 — Migrate the 55 active compiled verifier sources

This is a mechanical migration, not a semantic rewrite.

#### Rust: 49 files

For each active standalone verifier project:

1. identify what it proves;
2. identify the production crates it exercises;
3. move the compiled source to an appropriate normal Cargo target;
4. reproduce exact existing behavior;
5. make the historical/semantic runner invoke the new target;
6. compare output against the same frozen expectations;
7. delete the old standalone Cargo project only after equivalence is proven.

Possible destinations include:

- owning crate integration tests;
- normal test binaries;
- `searchkernel-testkit`;
- a small dedicated verifier-support crate if actual shared responsibility justifies it.

Do not create one giant verifier crate merely to centralize files.

#### C: 4 files

Move current-implementation/native ABI probes into explicit normal native test/verifier targets.

They should inherit canonical compiler flags, headers, ABI, adapter linkage, and runtime behavior.

#### C++: 2 files

Move the two SearchReply/current-implementation probes into the canonical native test graph.

#### Uncertain compile fixture: 1 file

Prove whether `m-api.rs` is still a live contract, then migrate or delete it.

Completion criterion: active Search Kernel verifier code no longer needs a forest of standalone compilation worlds under `reference/vespa`.

---

### Phase 5 — Leave the 14 oracle sources alone

Do not move the direct Vespa oracle sources merely because they are compilable.

They exist to provide independent behavioral evidence and should remain separated from Search Kernel's production implementation path.

Do not spend this convergence project normalizing every one-off oracle build script unless a concrete maintenance problem requires it.

Live-oracle execution and golden generation **must require explicit maintainer opt-in**. Remove environment-dependent `auto` behavior from the normal verification contract: ordinary verification replays committed frozen expected behavior even when a pinned Vespa source checkout happens to be available.

Completion criterion: ordinary development, verification, CI, and Universe neither compile nor automatically execute direct oracles; only an explicit maintainer authoring command/path may do so.

---

### Phase 6 — Collapse verifier-specific SDK/runtime setup

Once current-implementation verifier programs are normal build targets, they should use the same native artifact/runtime model as other Search Kernel consumers.

Verifier code should stop independently constructing:

- `LD_LIBRARY_PATH`;
- normal SDK locations;
- ordinary adapter locations;
- normal compiler linker flags;
- a separate verifier-native SDK/cache universe.

Exceptions are allowed for genuinely special tooling such as:

- sanitizer runtime libraries;
- intentionally independent direct Vespa oracle execution;
- explicit loader/runtime-negative tests.

The normal Search Kernel runtime model must remain consumer-safe and must not depend on these exceptions.

---

### Phase 7 — Converge semantic verifier orchestration

Every active verifier gets one public semantic command:

```bash
./dev verify <name>
```

A verifier runner may:

- choose scenarios;
- prepare temporary data;
- invoke an already-defined build target;
- canonicalize output;
- compare against frozen expected behavior;
- report PASS/FAIL.

It should not independently:

- invent a Cargo workspace;
- choose the normal compiler;
- construct the normal Search Kernel runtime;
- build a private normal SDK.

"Verifier" becomes a semantic execution lane, not a separate compilation architecture.

---

### Phase 8 — Reduce Universe to orchestration

Universe should only:

- enumerate canonical checks;
- invoke them;
- time them;
- record their result;
- summarize failures;
- print exact reproduction commands.

For example:

```text
FAIL: vespa.ranking
REPRO: ./dev verify vespa.ranking
```

Universe must not independently choose toolchains, compile verifier programs, construct normal native SDKs, or establish a different runtime model.

---

### Phase 9 — Simplify bootstrap

Normal repository correctness should be defined inside the canonical image.

The supported host contract is:

- Git;
- Docker/container runtime;
- repository authentication/access (normally GitHub CLI);
- ordinary shell/filesystem/worktree support.

Rust, C/C++, Python, Node, PHP, Ruby, FlatBuffers, and ordinary build dependencies belong to the canonical image for the normal workflow.

`./dev doctor` should converge on validating/repairing this host boundary plus the ability to enter the canonical environment. Host-native development may remain useful as an explicitly advanced workflow, but its toolchain bootstrap is not part of the ordinary success criterion and must not remain a second definition of correctness.

Simplify only after the canonical image path can build, test, verify, and perform remote dispatch without relying on host language toolchains.

---

### Phase 10 — Implement exact-SHA remote test/verify execution

Implement the remote execution contract from section 13 as a first-class migration phase rather than leaving it implicit in the Definition of Done.

Required deliverables:

1. one bounded on-demand workflow accepting exact revision, operation (`test|verify`), and known target;
2. `./dev test --remote <scope>` and `./dev verify --remote <lane>`;
3. validation that the requested commit is remotely available;
4. default refusal when the local working tree/index does not match the committed state being dispatched;
5. deterministic capture of the dispatched run identity;
6. wait/watch behavior, failed-step log surfacing, and result propagation;
7. an explicit self-hosted-runner trust policy with authorized actors, read-only permissions, no release/write secrets, and a stated policy for untrusted/fork revisions;
8. agent documentation requiring exact-head reruns after every fix.

Completion criterion: an agent without local native capacity can push an exact commit, run one canonical remote command, and receive trustworthy evidence for that exact commit without learning GitHub Actions internals.

---

### Phase 11 — Make release qualification a first-class canonical path

Implement:

```bash
./dev release-check
```

as the authority for shipability of one exact revision.

It must:

1. establish the exact revision being qualified;
2. run or consume the required exact-revision repository/source qualification according to release policy;
3. build the exact artifact set described by the release tooling;
4. validate the release manifest/checksums/provenance;
5. enumerate supported rows from `docs/release-support.json`;
6. execute artifact-level qualification for every supported row;
7. include ABI, persistence/backward-compatibility, native-runtime/relocatability, generated-artifact, and provenance checks required by the release contract;
8. fail if a supported row lacks executable evidence;
9. avoid depending on the name/content of a particular CI workflow YAML file.

At minimum, Rust embedded qualification must build and **execute** a downstream project from the actual distributable Rust bundle in a clean external location, not merely `cargo check` the bundle or consume the repository by path.

Completion criterion: `./dev release-check` is sufficient evidence for the statement "this exact revision's supported artifacts are qualified for release", while `./dev universe` remains the exhaustive repository/source qualification command.

---

### Phase 12 — Delete compatibility machinery

Deletion comes last.

Likely candidates include:

- standalone active verifier Cargo workspaces;
- duplicate verifier Cargo lockfiles;
- ad-hoc normal native-probe compiler invocations;
- verifier-specific normal adapter/runtime setup;
- verifier-specific Cargo target/cache topology;
- duplicate CI Docker/build setup;
- obsolete resolver usage;
- compatibility launchers that no longer serve a supported path.

Every deletion must answer:

> What canonical component now owns this responsibility?

If the answer is unclear, the old path is not ready to be removed.

---

## 15. Migration rules

### Rule 1 — Delegate before rewriting

Prefer:

```text
new canonical route
        |
        v
existing proven implementation
```

then simplify underneath.

### Rule 2 — Move build ownership, not test meaning

A verifier must prove the same thing after its executable source moves.

Do not opportunistically redesign semantic contracts during build convergence.

### Rule 3 — Centralize decisions, not syntax

Duplicating a few harmless shell commands is less dangerous than having two components independently decide:

- which SDK;
- which native base;
- which adapter;
- which compiler;
- which Cargo target root;
- which runtime library.

Centralize architectural authority, not text merely for aesthetic purity.

### Rule 4 — Preserve oracle independence

Direct Vespa reference paths must remain independent enough to avoid testing Search Kernel against itself.

### Rule 5 — Build once, run many

Use Cargo/native build caching rather than constructing a separate verifier artifact cache unless a measured problem requires one.

### Rule 6 — Keep CI intentionally minimal during convergence

Strong verifiers remain available for manual, targeted, agent-triggered, and release qualification use.

Do not re-enable the historical all-verifier PR matrix as a side effect of making lanes easier to run.

### Rule 7 — Preserve consumer correctness before deleting runtime machinery

The development system is allowed to simplify only if every supported row in `docs/release-support.json` remains qualified through its actual distributable artifact outside the repository environment.

### Rule 8 — Normal verification never authors truth

Normal test/verify/Universe execution consumes frozen reference behavior. Live oracle execution and golden regeneration require explicit maintainer intent.

### Rule 9 — Validate authorities, not obsolete wiring

Architectural/contract tests should assert public commands, artifact contracts, provenance, and safety invariants. They should not freeze obsolete workflow filenames or YAML implementation details once those are no longer authorities.

---

## 16. Explicit non-goals

This project does **not**:

- replace Cargo;
- replace Docker;
- replace GitHub Actions;
- adopt Bazel, Nix, Buck2, or another general build framework;
- rewrite `./dev` as `cargo xtask` merely for style;
- redesign the Search Kernel API;
- redesign the native ABI without separate justification;
- rebuild Vespa packaging from scratch unless the P0 distribution spike proves a smaller closure is materially better;
- rewrite verifier semantics;
- rewrite frozen reference fixtures;
- eliminate independent Vespa behavioral oracles;
- invent a verifier DSL or YAML build language;
- erase M0-M14 historical reference material;
- restore the old large PR CI matrix.

---

## 17. Reviewer guide

An engineer unfamiliar with Search Kernel should be able to review the migration with these questions.

### Public interface

Is ordinary repository work performed through `./dev`?

### Build graph

Are active Rust and native verifier programs visible to the normal repository build machinery?

### Consumer runtime — P0

For every supported row in `docs/release-support.json`, is the **actual distributable artifact** exercised according to its runtime contract?

For Rust embedded/native specifically, can a clean external project built from the distributable Rust artifact execute with `LD_LIBRARY_PATH` and repository-native overrides unset?

### Runtime identity — P0

Can a binary accidentally use another worktree's/build generation's Search Kernel native runtime?

### Native distribution — P0

Have we deliberately chosen between mostly-static and relocatable-shared native closure models rather than inheriting the current layout accidentally?

### `reference/vespa`

Does it primarily contain behavioral reference material, oracle provenance, and lightweight orchestration rather than a parallel normal build system?

### Oracle independence

Does the direct Vespa path remain independent from Search Kernel implementation code?

### CI

Does CI call the same canonical operations developers use?

### Universe and release qualification

Does Universe merely orchestrate canonical source/repository checks and print exact repros?

Does `./dev release-check` separately build and qualify the actual supported artifact set for one exact revision?

### Cache behavior

Do repeated compatible builds in the same worktree/environment reuse mutable output?

Can two concurrent unrelated worktrees run without sharing a writable Cargo target tree?

### Remote verification

Can an agent that cannot run a specialist check locally execute the same canonical check remotely against the exact pushed SHA?

Does GitHub Actions merely invoke the canonical `./dev` operation rather than reimplementing it?

Does remote execution refuse ambiguous dirty-worktree handoff by default and enforce the stated authorized-actor/self-hosted-runner trust policy?

### Complexity

Did the change remove an authority, or create another one?

---

## 18. End-state developer experience

A developer should need approximately this model:

### Machine setup

```bash
./dev doctor
```

### PR gate

```bash
./dev ci
```

### Build

```bash
./dev build
```

### Tests

```bash
./dev test --list
./dev test rust
./dev test python
```

### Specialist verification

```bash
./dev verify vespa.ranking
```

### Specialist verification on remote infrastructure

```bash
git push
./dev verify --remote vespa.ranking
./dev test --remote rust
```

Remote output identifies the exact pushed SHA and GitHub Actions run, waits for completion, and returns the remote operation's status.

### Full repository/source qualification

```bash
./dev universe
```

### Release qualification

```bash
./dev release-check
```

`release-check` builds and validates the actual supported artifact set; it is not merely another name for `universe`.

### Failure reproduction

```text
FAIL: vespa.ranking
REPRO: ./dev verify vespa.ranking
```

They should not need to know:

- which historical verifier Cargo workspace exists;
- where its lockfile lives;
- how a native probe is compiled;
- which Vespa test linker command was harvested;
- where a verifier SDK was synthesized;
- which `LD_LIBRARY_PATH` incantation makes a normal Search Kernel binary run.

A downstream user should need even less: the supported Search Kernel dependency/package should simply produce a runnable program.

---

## 19. Definition of done

The convergence project is complete when:

1. `./dev ci` exactly reproduces the PR gate.
2. GitHub Actions delegates the gate to `./dev`.
3. Normal build/test/verify operations use one canonical development environment.
4. Compatible Cargo/native outputs survive repeated container invocations.
5. Warm edit-build-test performance has no material regression.
6. The 55 active compiled verifier sources are owned by normal Rust/native build machinery.
7. The 14 oracle sources remain explicitly outside ordinary build/test requirements.
8. The uncertain compile fixture has been justified and migrated, or removed.
9. Active verifier Rust code no longer requires a forest of standalone Cargo workspaces under `reference/vespa`.
10. Current-implementation C/C++ probes are explicit normal native targets rather than ad-hoc compiler invocations.
11. Normal verifier runners no longer build private copies of the normal Search Kernel runtime environment.
12. `reference/vespa` primarily owns specification, scenarios, fixtures, frozen behavior, source traces, and oracle provenance.
13. Every active verifier has a canonical `./dev verify <lane>` command.
14. Universe invokes canonical commands only.
15. Every Universe failure prints the exact command that reproduces it.
16. Oracle compilation/golden generation is an explicit maintainer operation rather than an ordinary build/test dependency.
17. A supported external Rust consumer builds and runs with `LD_LIBRARY_PATH` unset.
18. Consumer execution does not require repository-local Cargo configuration or `./dev`.
19. Runtime identity cannot silently cross worktrees or incompatible build generations.
20. The native payload is relocatable according to its supported distribution contract.
21. The P0 mostly-static-vs-shared native-distribution investigation is documented with evidence.
22. Existing native SDK provenance guarantees remain intact.
23. Strong verifiers remain available outside mandatory PR CI.
24. `./dev test --list` and `./dev verify --list` expose the finite canonical test/verifier vocabulary, with lightweight documentation mapping major change areas to relevant checks.
25. `./dev verify --remote <lane>` and `./dev test --remote <scope>` provide explicit remote execution for agents/developers who cannot run a relevant check locally.
26. Remote execution checks out and verifies the exact pushed commit SHA, never merely a mutable branch reference.
27. Remote execution refuses dirty/ambiguous local handoff by default when the working tree/index differs from the commit being dispatched.
28. Remote execution ultimately invokes the same canonical `./dev verify <lane>` or `./dev test <scope>` command as local execution.
29. One generic bounded on-demand workflow handles remote `verify` and `test`; GitHub Actions contains no verifier-specific semantics.
30. Remote commands wait for the exact dispatched run, surface its run identity and failure logs, and propagate its result.
31. Remote verification has an explicit authorized-actor/self-hosted-runner trust policy, uses read-only permissions by default, exposes no release/write secrets, and defines handling for untrusted/fork revisions.
32. Arbitrary remote shell execution is not exposed.
33. Repository agent guidance documents the exact-SHA remote verification protocol and prohibits claiming verification from older commits.
34. Local canonical-container development preserves compatible writable Cargo/native incremental state across invocations in a worktree-scoped/environment-scoped namespace; unrelated worktrees do not share writable target state.
35. `./dev doctor` validates the minimal supported host boundary, while ordinary language/compiler toolchains remain owned by the canonical image.
36. `./dev build` has one explicit meaning: canonical supported product build in the canonical development environment.
37. Normal verification and Universe never automatically enter live-oracle mode; live oracle execution/golden generation require explicit maintainer opt-in.
38. `docs/release-support.json` is the machine-readable authority for supported downstream surfaces.
39. `./dev release-check` builds the actual release artifact set and provides artifact-level executable evidence for every supported release row.
40. Rust embedded release qualification builds and executes a clean external consumer from the actual distributable Rust artifact, not merely from a repository path.
41. Release qualification checks public commands/artifacts/provenance rather than depending on obsolete CI workflow filenames or YAML implementation details.
42. Old compatibility/build paths are removed only after their replacements are proven.
43. `BUILDING.md` can explain the normal developer workflow in roughly one screenful.
44. An engineer unfamiliar with Search Kernel can identify where Rust compilation, native compilation, runtime packaging, release qualification, and behavioral references live without discovering another hidden build system.

The final architecture should be boring:

> `./dev` owns developer workflow.  
> The canonical image owns the development toolchain.  
> Cargo owns Rust compilation.  
> Native build tooling owns native compilation.  
> The build/package layer makes consumer artifacts runnable.  
> Consumers do not depend on `./dev`.  
> Active verifier programs use the normal build graph.  
> `reference/vespa` owns behavioral evidence and oracle provenance.  
> Per-worktree mutable build state keeps local iteration warm without cross-agent contamination.  
> Universe orchestrates repository/source qualification.  
> Release-check builds and qualifies the actual supported artifact set.  
> Explicit remote verification dispatches authorized exact-SHA `./dev` operations.  
> CI calls `./dev`.

That is the project.
