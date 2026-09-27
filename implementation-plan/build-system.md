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

## 2. Core architectural invariant

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
| Full qualification | `./dev universe` |
| PR gate | `./dev ci` |
| GitHub Actions | thin invocation of `./dev` |

The plan is successful only if it **reduces the number of independent authorities**.

---

## 3. P0 consumer-runtime design constraints

The development environment is not the product runtime.

Search Kernel embeds Vespa. Therefore the native implementation required by Search Kernel must ultimately reach downstream users in some form. A successful development build is not enough; the resulting consumer artifact must work outside this repository.

### P0.1 — Consumer artifacts must be self-sufficient

A supported Search Kernel native-linked Rust consumer must not rely on:

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

A focused qualification must build a consumer outside the Search Kernel workspace and execute it with `LD_LIBRARY_PATH` unset.

Current `main` already has consumer-safe runtime work from PR #745, including an external consumer smoke path. This plan treats that behavior as a required invariant. The current mechanism is not necessarily the final mechanism.

---

## 4. P0 native-distribution decision

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

## 5. Why not replace everything with Nix, Bazel, Buck2, xtask, or just?

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

## 6. Canonical development environment

Normal build/test/verification should run in the canonical Search Kernel image.

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

## 7. Build/cache performance constraint

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

CI may isolate writable build outputs per job. Local development should optimize strongly for the warm edit-build-test loop.

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

## 8. Audit: `reference/vespa`

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

### 8.1 Active normal-verification sources: 55

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

### 8.2 Oracle/golden-authoring sources: 14

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

### 8.3 One Rust source requires investigation

`reference/vespa/api-contract/src/bin/m-api.rs` is described as a compile-only stable API fixture, but it was not found in the current normal milestone/product verifier entrypoints inspected during the audit.

Before migrating it:

```text
prove active -> migrate
prove obsolete -> delete
```

Do not preserve dead build machinery merely for symmetry.

### 8.4 Important wording

`reference/vespa` is inside the repository, so the issue is not that it is "outside the source tree".

The precise problem is:

> `reference/vespa` has become both a specification/reference corpus and a secondary mixed-language build tree outside the normal product build graph.

---

## 9. Verifier versus oracle

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

## 10. Target role of `reference/vespa`

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

## 11. Public command surface

The normal developer interface should converge toward:

```bash
./dev doctor
./dev build
./dev ci

./dev test [scope]
./dev lint [scope]
./dev verify <lane>

./dev universe
./dev release-check
```

Advanced maintainer commands may continue to exist for image publishing, SDK inspection, protocol generation, oracle/golden generation, worktrees, and release maintenance.

A developer should not need those commands to understand an ordinary build.

---

## 12. Migration plan

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

Run the mostly-static closure investigation described in section 4.

Preserve the current #745 consumer-safe runtime mechanism while investigating alternatives.

Whatever model wins must prove:

1. a clean downstream Cargo consumer outside the workspace builds;
2. it runs with `LD_LIBRARY_PATH` unset;
3. it does not depend on repository Cargo configuration;
4. runtime identity cannot silently cross worktrees/build generations;
5. required native payload is relocatable according to its supported distribution contract;
6. native SDK/provenance guarantees remain intact.

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

---

### Phase 3 — Make `./dev` a thin router

`./dev` should primarily:

1. parse the requested operation;
2. decide host versus canonical environment;
3. dispatch the existing implementation.

Do not rewrite the entire script.

Move implementation responsibility downward into Cargo/native/test tooling where it belongs.

Keep compatible target/cache state persistent across container invocations.

Completion criterion: the routing model is obvious from the top of `./dev`.

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

Prefer an explicit maintainer operation for live-oracle/golden generation.

Completion criterion: ordinary development, verification, and CI do not compile oracles.

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

The host should eventually require only enough tooling to enter that environment, conceptually:

- Git;
- Docker/container runtime;
- repository access;
- minimal filesystem/worktree support.

Host-native development may remain useful as an advanced workflow. It should not remain a second definition of correctness.

Simplify it only after the canonical path is established.

---

### Phase 10 — Delete compatibility machinery

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

## 13. Migration rules

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

The development system is allowed to simplify only if external consumers remain able to build and execute successfully outside the repository environment.

---

## 14. Explicit non-goals

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

## 15. Reviewer guide

An engineer unfamiliar with Search Kernel should be able to review the migration with these questions.

### Public interface

Is ordinary repository work performed through `./dev`?

### Build graph

Are active Rust and native verifier programs visible to the normal repository build machinery?

### Consumer runtime — P0

Can a clean external Cargo consumer run with `LD_LIBRARY_PATH` unset and without Search Kernel repository configuration?

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

### Universe

Does Universe merely orchestrate canonical commands and print exact repros?

### Cache behavior

Do repeated compatible builds reuse output?

### Complexity

Did the change remove an authority, or create another one?

---

## 16. End-state developer experience

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
./dev test
./dev test python
```

### Specialist verification

```bash
./dev verify vespa.ranking
```

### Full qualification

```bash
./dev release-check
```

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

## 17. Definition of done

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
24. Old compatibility/build paths are removed only after their replacements are proven.
25. `BUILDING.md` can explain the normal developer workflow in roughly one screenful.
26. An engineer unfamiliar with Search Kernel can identify where Rust compilation, native compilation, runtime packaging, and behavioral references live without discovering another hidden build system.

The final architecture should be boring:

> `./dev` owns developer workflow.  
> The canonical image owns the development toolchain.  
> Cargo owns Rust compilation.  
> Native build tooling owns native compilation.  
> The build/package layer makes consumer artifacts runnable.  
> Consumers do not depend on `./dev`.  
> Active verifier programs use the normal build graph.  
> `reference/vespa` owns behavioral evidence and oracle provenance.  
> Universe orchestrates.  
> CI calls `./dev`.

That is the project.
