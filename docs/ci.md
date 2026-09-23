# CI verification architecture

Status: proposed, to be implemented incrementally.

## Goal

Make GitHub Actions capable of running the complete SearchKernel test and verifier
suite, including real-Vespa/native verification, while keeping pull-request CI
change-aware and reasonably fast.

The end state should make CI authoritative for required verification. Local
pre-commit checks and coding-agent reports are useful evidence, but CI must not
depend on them having run.

## Trust model

Verification selection is a repository contract, not agent discretion.

For a pull request, the required verification set is the union of:

1. repository policy derived from the actual base-to-head diff;
2. issue/acceptance requirements expressed as repository-owned verification
   profiles where practical; and
3. additional focused tests requested or run by an agent/reviewer.

Agent- or issue-supplied verification may add evidence. It must never remove
verification required by repository policy.

### Trusted policy boundary

A pull request must not control the policy that decides whether that same pull
request has been sufficiently tested.

CI therefore classifies a candidate diff using verification policy from the
trusted base revision. This preserves the existing exact-head design in
`.github/ci/exact_head.py`.

If verification policy, CI orchestration, or verifier infrastructure changes,
the old/base policy remains authoritative for that pull request. The candidate
policy's self-tests are run in addition, and the change receives a conservative
or full verification profile.

Local tooling may use the working-tree policy for developer feedback, but it is
not authoritative CI evidence.

## Fail-closed selection

Path classification must not silently turn an unknown behavioral path into "no
tests required".

The policy should distinguish:

- explicitly non-behavioral paths that are safe to exempt;
- known behavioral paths with mapped verification profiles;
- verification-policy/orchestration changes; and
- unknown source/build/tooling paths.

Unknown behavioral, source, build, or tooling paths select a conservative/full
profile until explicitly classified.

Selection also needs semantic dependency rules rather than path patterns alone.
For example, schema, ABI, dependency/build-system, persistence, generated-code,
and native changes may fan out to several verification profiles.

## Verification profiles

The repository should own named verification profiles and the commands that
implement them. The selector chooses profile IDs; CI executes those profiles.

The planner and executor must not maintain independent hand-written meanings for
the same profile.

Expected profile families include, subject to inventory:

- baseline Rust formatting/lint/unit tests;
- Rust behavioral tests;
- Python client tests;
- TypeScript client tests;
- PHP client tests;
- protocol/generated-artifact checks;
- API conformance;
- capability/client contracts;
- ABI verification;
- persistence compatibility;
- issue-queue/tooling verification;
- native real-engine verification;
- native client/gRPC end-to-end verification;
- Vespa milestone/oracle verification;
- verification-policy self-tests.

The exact inventory and dependency graph are intentionally not frozen here yet.
They must be derived from the repository's current executable tests and
verifiers before implementation.

## Developer and agent interface

The intended user-facing interface is approximately:

```text
./dev test-plan --base <base> --head <head>
./dev test-affected --base <base> --head <head>
./dev test-all
```

`test-plan` explains what the change requires.

`test-affected` runs the repository-selected verification for the change.

`test-all` runs the complete verification DAG without change-based omission.

Names may change during implementation, but there should be one repository-owned
profile model behind pre-commit, developer tooling, agent guidance, and CI.

Coding agents should run the plan and report exact commands/results. CI
independently recomputes the authoritative plan from the trusted base policy and
the actual candidate diff.

Lint execution follows the same repository-owned-command rule. The canonical
workspace Clippy profiles run through `./dev lint`; CI requests a diagnostics
directory so failures produce a repair packet with rendered output,
machine-readable Cargo/Clippy diagnostics, a lint-vs-build failure
classification, and the exact local reproduction command. CI uploads that
packet when Clippy fails and also writes the summary to the Actions job summary.

Issue acceptance requirements should prefer repository-owned profile IDs or
clear acceptance behavior rather than arbitrary shell supplied by issue text.
Arbitrary agent-reported commands remain evidence only.

## CI execution model

SearchKernel owns the **verification authority** but should not build a generic
task scheduler, cache engine, Rust test runner, or distributed compilation
system.

The intended division of responsibility is:

```text
SEARCHKERNEL-OWNED TRUST LAYER

trusted base policy
       |
       v
base..candidate diff
       |
       v
fail-closed semantic classification
       |
       v
required verification task IDs
       |
       v
exact-head / base-drift authorization


COMMODITY EXECUTION LAYER

moonrepo
    task graph / dependencies / parallel execution / task caching
       |
       +---- cargo-nextest
       |         Rust test execution / filtering / sharding / archives
       |
       +---- sccache
       |         Rust and C/C++ compilation cache
       |
       +---- repository verifier commands
                 native / protocol / clients / persistence / oracle / E2E
```

The trusted SearchKernel selector answers **what verification is mandatory**.
moonrepo executes the resulting repository-owned tasks. moonrepo's own affected
calculation may be useful for local developer feedback and execution
optimization, but it is not authoritative for deciding whether a candidate has
tested itself sufficiently.

The canonical task graph should therefore be declarative and mechanical. A
profile/task owns one command and may depend on other tasks, while the trusted
selector returns task IDs plus reasons. GitHub Actions should not maintain a
second handwritten meaning for those tasks.

For Rust test-heavy tasks, prefer cargo-nextest over bespoke test scheduling.
Where one compiled Rust test corpus is executed on several workers, prefer
nextest archives/partitions so CI compiles once and executes many rather than
rebuilding the same tests independently on every shard.

Use sccache from the beginning for supported Rust and C/C++ compilation. This is
baseline CI infrastructure for SearchKernel's expensive native/Rust build, not a
late optimization.

Verification still runs against candidate code even though authoritative
selection is made by trusted base policy.

CI must continue to guard exact-head evidence. Where practical, final acceptance
must also protect against base drift by testing the current merge candidate or
merge result, or by requiring re-verification after the base changes. Post-merge
`main` verification remains part of closure for substantive work.

## Native runner provisioning

Normal CI must not rebuild Vespa as part of ordinary test execution.

Self-hosted runners should be provisioned with immutable, versioned native
prerequisites. The simplest initial implementation may use rsync because the
large prerequisites change rarely.

Provisioning is expected to include everything required by the real verifier,
not merely the SearchKernel native SDK. The current verifier still requires a
clean pinned Vespa Git checkout through `VESPA_ROOT`, and milestone/oracle
verification may require prebuilt oracle artifacts.

An initial runner can therefore carry:

- the immutable SearchKernel native base/SDK and runtime library closure;
- a clean configured Vespa checkout at the pinned revision;
- required prebuilt oracle artifacts;
- required compiler/toolchain/system dependencies.

Later, this may be reduced to a purpose-built immutable verifier bundle if the
verifiers are changed to prove equivalent provenance without requiring the full
Vespa checkout.

Provisioned state must have an identity and be validated before use: at minimum
the Vespa revision, architecture/build environment, native ABI identity, and
artifact manifests/checksums as applicable. Prefer versioned immutable
directories with an atomic `current` pointer rather than mutable in-place
state.

Shared prerequisites should be read-only/immutable. Runtime data, persistence
directories, sockets, generated temporary state, and other mutable test state
must be job-private so concurrent Actions jobs cannot contaminate one another.

## Verifier independence

A candidate that changes a verifier, expected result, frozen fixture,
classification rule, or CI orchestration deserves special handling because it
may otherwise weaken the evidence used to approve itself.

Where compatible, run trusted/base verifier logic against candidate
implementation code. Where a deliberate contract change makes that impossible,
require explicit review of the verifier/fixture delta and conservative/full
verification.

Passing a candidate-modified verifier is not by itself independent proof that
the candidate is correct.

## Full-suite inventory

Before implementing `test-all`, inventory every current test and verifier
entry point and determine its real coverage and prerequisites.

Do not assume umbrella commands are complete from their names. In particular,
the cumulative Vespa milestone/integration chain must be inspected
mechanically. At the time this document was started, the integration verifier
and newer milestone verifiers do not necessarily mean the same thing, so the
authoritative full-suite definition must be explicit.

The inventory should record for every executable verification unit:

- stable profile ID;
- command/entry point;
- paths/semantic changes that require it;
- dependencies on other profiles;
- whether another verifier genuinely subsumes it;
- native/Vespa/oracle/toolchain prerequisites;
- expected runtime and useful parallelization boundary;
- mutable state/cache behavior;
- whether it can run on GitHub-hosted or requires a self-hosted runner;
- whether it is safe for PR CI, nightly/full CI, or both;
- whether its implementation is trusted from base or candidate revision.

This inventory is the next implementation step.

## Current verification inventory

Inventory basis: repository `main` at `d89207dda9f76dd558f67883176875ef6fa06f90`.
Runtime classes below are static estimates from the commands and prerequisites,
not wall-clock measurements. Actual timing belongs in Phase 7.

### Canonical profile families

| Profile ID | Canonical/current entry point | Current trigger/semantic scope | Dependencies and subsumption | Prerequisites / runner | Mutable state / parallelization | Current CI | Trust boundary | Runtime class |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `rust-non-native` | `cargo test --workspace --all-targets --no-default-features` | Current `rust` lane: root Cargo metadata, `.cargo/**`, `api/**`, `core/**`, `searchkerneld/**`, `dev` | Independent baseline; does not prove native/all-features behavior | Rust + pinned FlatBuffers; any Linux runner | Cargo target/cache only; separate target roots per job | PR behavioral, unit, nightly | Candidate code; lane selection must come from trusted base | medium |
| `rust-quality` | `cargo fmt --all --check`; `./dev lint --profile rust-clippy-non-native` | Broad Rust/build changes | Independent baseline; repeated inside several native milestone verifiers via `run_workspace_quality` with all features | Rust; native all-feature variant needs native SDK | Cargo target only | Nightly; pre-commit | Candidate code | medium |
| `python-client` | `./dev test python` | Current `python` lane: `api/**`, `clients/python/**`, client contracts and Python generation/check tooling | Includes Python generated checks plus unittest discovery; ordinary discovery skips real native gRPC E2E unless opt-in | Python runtime + package deps + FlatBuffers tools | Temp/package state; isolate venv where possible | PR behavioral, nightly | Candidate code; base-owned selection | fast-medium |
| `typescript-client` | `./dev test typescript` | Current `typescript` lane: `api/**`, TS client, client contracts and TS generators | Includes generation regression, generated check, typecheck, Vitest, package verification | Node 22, Rust/FlatBuffers for semantic generation, npm deps | `dist`, generated/temp files; safe with per-job checkout | PR behavioral | Candidate code; base-owned selection | medium |
| `php-client` | `./dev test php` | Current `php` lane: `api/**`, PHP client, client contracts | Standalone first-slice test runner | PHP runtime | Minimal mutable state | Client-capabilities workflow; not behavioral exact-head executor | Candidate code; base-owned selection | fast |
| `protocol-generated` | `./dev check-protocol` | Current `protocol` lane: schemas, BFBS/generated Rust/Python/TS/RPC artifacts and generators | Runs BFBS, protocol, RPC, Python FlatBuffers/gRPC, TS generated checks | pinned flatc/FlatBuffers C++; Python codegen deps; Node for TS generated checks | Regeneration checks should not mutate committed tree | Nightly partially; pre-commit; not a dedicated exact-head job | Candidate artifacts; selection from base; policy/generator changes elevated | fast-medium |
| `api-conformance` | `./dev verify api-conformance` / `reference/api-conformance/verify.sh` | Current `api_conformance` lane only maps `reference/api-conformance/**` | `verify.sh -> verify-fixtures.sh -> validate_suite.py`, backend-free guard, then `searchkernel-conformance`; parameterized `verify-adapter.sh` is separate | Rust + Python; no Vespa | Temporary Cargo output | Unit workflow | Candidate verifier normally; verifier/expected changes require elevated review | medium |
| `client-contracts` | `./dev verify contracts` | Current `contracts` lane: clients, public Rust API, Vespa-engine capability docs/ledgers, contract tooling | Capabilities check/tests + client-contract check/tests + Python dependency contract | Python; executable E2E additionally needs Rust/Node/client deps | E2E starts processes/uses temp state through checker; job-local | Client-capabilities, nightly | Candidate code; contract/checker changes need elevated review | fast-medium |
| `abi` | `./dev verify abi` | Current `abi` lane: native headers/adapter/bindings and ABI tooling | Authority tests/check, ABI surface regression, checked-in bindings, layout, SDK-cache regression, header/linker-map surface comparison | Rust/rustfmt, system Python with clang bindings/libclang, C/C++ toolchain | Native cache regression uses shared cache semantics; must isolate temp paths | Dedicated ABI workflow, nightly subset | Candidate code; baseline header comes from trusted base for change comparison | medium |
| `persistence-static` | `tools/check-persistence-fixtures.sh`; `tools/check-persistence-fixture-immutability.sh <base>` | Current persistence path set and frozen fixture changes | Static/provenance part of `./dev verify persistence` | Shell/Python/Git | Reads frozen corpus; no shared mutable index | Dedicated persistence workflow | Immutability comparison must use trusted base | fast |
| `persistence-native` | `./dev verify persistence` | Persistence/storage semantics: schema snapshots, index facade, IR adapter, Vespa engine/bindings/native, persisted fixtures | Static checks + native `persistence_compatibility` + `persistence_fault_matrix_` + sensitivity checker | Full native prerequisite set | Creates/reopens indexes; job-private data dirs required | Explicitly local-only today; CI emits notice | Candidate code; frozen fixture changes elevated | heavy |
| `native-python-e2e` | `tools/run-python-native-e2e.sh` | Current pre-commit maps every `native` change to this E2E | Calls `./dev native-sdk ensure`, then opt-in `test_grpc_e2e.py` | Native SDK/base + searchkerneld build path + Python/gRPC | Daemon/socket/temp index state must be job-private | Not in Actions today | Candidate code; base-selected native requirement | heavy |
| `vespa-m14` | `./dev verify m14` / `reference/vespa/m14/verify-m14.sh` | Latest explicit M-series milestone | Cumulative: `M14 -> M13 -> M12 -> M11 -> M10 -> M9 -> M8 -> M7 -> M6 -> M5 -> M4 -> M3 -> M2 -> M1 -> M0`; includes M10 SearchReply + feature-values, M9 recursive grouping, M11 core/stress/sanitizers | Clean pinned Vespa Git checkout, native base/SDK, C/C++/Rust/Python, prebuilt direct C++ oracles for M12-M14 and other milestone-specific oracle assets, sanitizer runtimes | Very large mutable verifier target tree; stress/sanitizers must be isolated and are poor candidates for concurrent reuse of mutable dirs | Not in Actions | Frozen verifier/oracle changes require independent review; selection must be base-owned | very heavy |
| `vespa-integration` | `./dev verify integration` / `reference/vespa/i/verify-i.sh` | Integrated public product path | Runs `M13` (not M14) + `phase10`, then its own integration driver. Therefore it does **not** subsume M14 | Same native/Vespa prerequisites as M13 plus Phase 10 | Job-private index/config/data dirs | Not in Actions; current native pre-commit gate | Candidate implementation; verifier changes elevated | very heavy |
| `phase10-rust-e2e` | `./dev verify phase10` | Public Rust real-engine E2E | Independent of M14; integration invokes it after M13 | Full native SDK + pinned Vespa | Real index/runtime state; job-private target/data | Not in Actions | Candidate code; acceptance script changes elevated | heavy |
| `searchreply` | `./dev verify searchreply` | Search-result total-hits/coverage/degradation/issues semantics | Includes `total-hit-accuracy`; also included transitively by M10 and therefore M14 | Full native prerequisites; builds direct C++ oracle and a failpoint-enabled hardening SDK | Builds SDK/probes and native data; isolate target dirs | Not in Actions | Candidate code; oracle/expected changes elevated | heavy |
| `recovery` | `reference/vespa/recovery/verify-recovery.sh` | Product recovery matrix/repeat/missing-state behavior | Standalone; **not** invoked by `./dev verify`, M14, or integration | Full native prerequisites | Destructive/reopen-style state under `target/vespa-verifier/recovery`; must be job-private | Not in Actions | Candidate code; frozen expected/driver changes elevated | heavy |
| `m13-grouping-yql` | `reference/vespa/m13/grouping-yql/verify-grouping-yql.sh` | Additive grouping/YQL acceptance | Runs recursive grouping + canonical Python E2E; despite its location it is **not called by `verify-m13.sh`** and therefore not inherited by M14 | Full native prerequisites + Python client | Native/Python E2E state; job-private | Not in Actions | Candidate code; verifier changes elevated | heavy |
| `phase9` | `reference/vespa/phase9/verify-phase9.sh` | Historical/additive native ABI and selected vertical slices | Runs M13 + recursive grouping + ABI/debug-info and selected native/adapter tests; not exposed by `./dev verify`; overlaps later coverage but is not mechanically proven redundant | Full configured Vespa/native environment; may build SDK | Heavy build/temp state | Not in Actions | Candidate code; requires disposition before exclusion from full suite | very heavy |
| `c2-historical` | `reference/clients/c2/verify-c2.sh` | Historical C2 API/IR contract and no-Vespa dependency firewall | Standalone; not invoked by current `./dev verify contracts` or Actions | Rust only | Cargo output | Not in Actions | Candidate verifier; historical status must be explicit before omission | medium |
| `web-dashboard` | `npm --prefix searchkernel-web test`; `npm --prefix searchkernel-web run build` | Dashboard TypeScript/React code | Independent; not represented in current `exact_head.py` lanes or Actions workflows | Node/npm deps | Vite/Vitest build outputs; per-job checkout | Not in current CI | Candidate code; future frontend policy should be base-owned | medium |
| `frontend-harness` | `tools/frontend-harness/frontend-ci`, `frontend-verify`, `frontend-verifier-integrity`, `frontend-harness-selftest` | Future F0-F12 dashboard milestone acceptance | F0-F12 `verify.sh` entry points are intentionally NOT_IMPLEMENTED today; acceptance cannot pass and should not be placed in the current green full-suite root until activated | Python + frontend repo state; milestone-specific deps later | Writes review/result state under target; acceptance requires clean worktree | Not in Actions | Freeze commit/integrity boundary is authoritative when activated | currently non-runnable as acceptance |
| `verification-policy` | `python3 .github/ci/test_exact_head.py` | Current `verification` lane: pre-commit, selector/tests, `dev` | Tests selector/hook behavior; Actions classification intentionally runs trusted base helper | Python/Git; some self-tests fake Cargo/dev | Temporary Git repos | Behavioral classification job | **Base revision must remain authoritative for selection**; candidate self-tests are additive | fast |
| `issue-queue` | `./dev verify issue-queue` | Current `issue_queue` lane: queue checker/tests/workflow and related Python selection tooling | Unit tests; live checker needs GitHub access/token | Python; live check needs GitHub | No product state | Dedicated issue-queue workflow | Candidate checker; live repository state external | fast |

### Vespa/native closure graph

The current native acceptance graph is not one tree:

```text
M14
  -> M13
     -> M12
        -> M11
           -> M10
              -> M9
                 -> M8 -> M7 -> M6 -> M5 -> M4 -> M3 -> M2 -> M1 -> M0
              -> SearchReply
                 -> total-hit-accuracy
              -> M10 feature-values
           -> M11 core
           -> M11 stress/resources
           -> M11 ASan/UBSan + TSan
              (M9 also -> recursive grouping)

integration (i)
  -> M13
  -> Phase 10 public Rust E2E
  -> integration driver

standalone/additive native gates
  -> M13 grouping/YQL
       -> recursive grouping
       -> canonical Python E2E
  -> recovery
  -> phase9
       -> M13
       -> recursive grouping
       -> ABI/debug-info + selected vertical slices
  -> Python native gRPC E2E
```

Consequences:

- `verify integration` stopping at M13 means it cannot authorize M14.
- `verify m14` is the latest cumulative M-series gate, but it still does not
  include the standalone M13 grouping/YQL verifier, recovery verifier, Phase 10
  E2E, integration driver, Phase 9, or Python native gRPC E2E.
- M13 grouping/YQL is not called by M13 despite being located beneath
  `reference/vespa/m13/`.
- SearchReply and M10 feature-values are genuinely inherited by M14 through M10.
- Recursive grouping is inherited by M14 through M9, while M13 grouping/YQL
  adds a distinct Python/product path on top of it.
- Phase 9 overlaps later native acceptance heavily, but source inspection alone
  does not prove every selected Phase 9 vertical slice is subsumed. It must be
  explicitly retired or retained in the future full-suite root rather than
  silently omitted.

### Tooling and subsidiary executable entry points

The following executable checks/tests exist beneath `tools/` and are inputs to
the profile families above. Keeping this list explicit prevents a helper from
silently disappearing during future profile consolidation.

```text
check-capabilities.py
check-client-contracts.py
check-flatbuffers-bfbs.sh
check-flatbuffers-cpp.sh
check-ir-boundary.sh
check-issue-queue.py
check-layout.sh
check-native-abi-authorities.py
check-native-abi.sh
check-native-bindings.sh
check-native-sdk-hardening.sh
check-persistence-fixture-immutability.sh
check-persistence-fixtures.sh
check-persistence-sensitivity.sh
check-python-dependencies.py
check-python-flatbuffers-boundary.py
check-python-flatbuffers-generated.sh
check-python-grpc.sh
check-search-semantic-entry-points.py
check-searchkernel-protocol-generated.sh
check-searchkernel-rpc.sh
check-testkit-backend-free.py
check-typescript-generated.sh
check-vespa-bindings.sh
check_abi_surface.py
run-python-native-e2e.sh
test-check-native-abi.sh
test-flatbuffers-cpp-toolchain.sh
test-native-base-artifact-real.sh
test-native-base-artifact.sh
test-native-bindings.sh
test-native-build-hardening.sh
test-native-environment-fingerprint.sh
test-native-sdk-cache.sh
test-native-sdk-hardening.sh
test-native-sdk-packaging.sh
test-typescript-generation.sh
test_check_abi_surface.py
test_check_api_conformance_backend_free.py
test_check_capabilities.py
test_check_client_contracts.py
test_check_ir_boundary.py
test_check_issue_queue.py
test_check_native_abi_authorities.py
test_check_search_semantic_entry_points.py
test_python_dependency_contract.py
```

Additional executable acceptance entry points outside `tools/` are:

```text
reference/api-conformance/verify.sh
reference/api-conformance/verify-fixtures.sh
reference/api-conformance/verify-adapter.sh   # parameterized utility
reference/clients/c2/verify-c2.sh
reference/vespa/verify-through.sh
reference/vespa/i/verify-i.sh
reference/vespa/m0/verify-m0.sh ... reference/vespa/m14/verify-m14.sh
reference/vespa/m3/kstem/verify-kstem.sh
reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh
reference/vespa/m10/feature-values/verify-feature-values.sh
reference/vespa/m11/verify-m11-{core,stress,sanitizers}.sh
reference/vespa/m13/grouping-yql/verify-grouping-yql.sh
reference/vespa/phase9/verify-phase9.sh
reference/vespa/phase10/verify-phase10.sh
reference/vespa/recovery/verify-recovery.sh
reference/vespa/searchreply/verify-searchreply.sh
reference/vespa/searchreply/total-hit-accuracy/verify-total-hit-accuracy.sh
clients/php/tests/run.php
clients/typescript package scripts: typecheck, test, test:package, example:e2e
Python unittest discovery under clients/python/tests
searchkernel-web package scripts: build, test
tools/frontend-harness frontend-{ci,verify,accept,verifier-integrity,harness-selftest}
tools/frontend-harness/verifiers/F0/verify.sh ... F12/verify.sh
```

### Current CI coverage and gaps

Current Actions coverage is asymmetric:

- PR behavioral CI executes only Rust, Python, and TypeScript lanes even though
  the trusted classifier emits PHP, native, protocol, API-conformance,
  contracts, ABI, persistence, issue-queue, and verification signals.
- Native-sensitive PRs currently receive only a notice directing local
  verification; no real-Vespa lane runs in Actions.
- Persistence CI enforces frozen-corpus structure/immutability but explicitly
  omits native reopen/mutation/fault/sensitivity execution.
- ABI has a dedicated self-hosted workflow and is the strongest existing
  example of focused CI-level verification.
- Nightly is comprehensive only for non-native Rust/contracts/client E2E and a
  small ABI/cache subset; it is not a full product/native suite.
- `searchkernel-web`, recovery, M13 grouping/YQL, Phase 9, M14, integration,
  Phase 10, SearchReply as a direct gate, Python native gRPC E2E, and historical
  C2 are not represented by a complete Actions closure today.
- The F-series frontend harness is intentionally future/non-implemented and is
  not a current green acceptance dependency.

### Native prerequisite inventory

The real native verifier family currently requires more than a copied DSO set:

- the immutable native base/runtime closure;
- a SearchKernel adapter/header/manifest view built against that base;
- a **clean Git Vespa checkout at the exact pinned commit** because
  `require_pinned_vespa` inspects Git HEAD and dirtiness;
- configured Vespa build/header/dependency material used by adapter/oracle
  builds;
- direct C++ oracle binaries or the toolchain/source needed to build them;
- C/C++ compiler/linker tools, `nm`, `ldd`, `sha256sum`, `rg`, Python,
  Rust/rustfmt/clippy;
- ASan/UBSan/LSan/TSan runtimes for the M11 sanitizer lane;
- system Python + libclang bindings for ABI surface analysis;
- Node/npm and client dependencies for cross-language/native E2E paths.

Therefore rsyncing the existing configured/pinned Vespa tree plus immutable
native artifacts/oracles is the shortest initial route to a full native runner.
A smaller verifier bundle is a later optimization, not a prerequisite for CI
enablement.

### Isolation and concurrency ledger

- Immutable native base/toolchain/oracle inputs may be shared read-only.
- Cargo caches may be shared only through normal cache semantics and distinct
  target roots where build environments differ.
- Every native verifier that creates config/data/index state must use a
  job-private target/data directory.
- M11 stress/resource and sanitizer lanes should be separate heavy jobs; they
  mutate target trees and exercise process/thread/resource behavior.
- Persistence/recovery tests must never share index directories across jobs.
- gRPC E2E must use job-private sockets/ports/temp index state.
- Generated-artifact checks should compare against committed output without
  leaving generated candidate files shared across jobs.
- Frontend harness review/results state belongs under job-local `target/`.

### PR/full-suite suitability

Recommended inventory-level placement before optimization:

- **Every PR baseline:** rust non-native, verification-policy self-tests when
  relevant, and any selected fast protocol/client/contract/ABI/static
  persistence profiles.
- **Selected PR heavy:** native real-engine, native gRPC E2E, native
  persistence/recovery, milestone/oracle gates according to trusted selection.
- **Main/nightly full:** M14 cumulative, integration/Phase 10, M13 grouping/YQL,
  recovery, native Python gRPC E2E, all non-native profiles, and any retained
  Phase 9/C2 historical gates after explicit disposition.
- **Nightly or dedicated heavy runner:** M11 stress/resources and sanitizers if
  PR latency becomes unacceptable; correctness-sensitive M11 changes should
  still select them before merge.
- **Excluded until activated:** F0-F12 frontend milestone acceptance because the
  repository explicitly marks the initial verifiers NOT_IMPLEMENTED.

## Fail-closed selection policy

Phase 2 makes the trusted selector conservative before the moon task graph is
introduced.

The policy uses these rules:

- known lane patterns select their existing verification families;
- portable FlatBuffer schema changes fan out to Rust, all current language
  clients, native behavior, generated-protocol checks, API conformance,
  contracts, and persistence because the canonical schema can affect stored and
  executable semantics;
- RPC/protocol-tooling changes fan out across Rust/Python/TypeScript protocol
  and contract checks without automatically implying persistence/native
  semantics;
- shared client-contract changes fan out to all client lanes plus contracts;
- generated wire/RPC artifacts fan out across Rust/Python/TypeScript protocol
  and contract checks so one generated surface cannot drift independently;
- native behavior changes fan out to Rust, native, persistence, and contracts;
- ABI-sensitive native/tooling changes additionally select ABI verification;
- persistence/storage changes select Rust, native, and persistence;
- root Cargo/toolchain metadata selects every current lane because it can alter
  workspace membership, features, native linkage, or code generation;
- verification/task-control changes under `.githooks/**`, `.github/ci/**`,
  `.github/workflows/**`, `.moon/**`, root moon configuration, or `dev`
  select every current lane;
- any path not recognized by a lane and not explicitly exempted selects every
  current lane.

The explicit non-behavioral exemptions are deliberately narrow: repository and
nested README/AGENTS documentation, Markdown/text files under `docs/**`,
Markdown under `implementation-plan/**` and `audits/**`, license/notice and
ignore files. Common local scratch suffixes such as `.tmp`, `.log`, and
`.swp` are exempted only by pre-commit's **untracked-file** check; once tracked,
they fail closed like any other unknown file. Behavioral ledgers/configuration
already mapped by a known lane (for example `docs/capabilities.json`) still
select that lane. Unknown non-document files under documentation directories
fail closed.

CI continues to execute the classifier copied from the trusted PR base revision.
The authoritative diff is produced with rename detection disabled:

```text
git diff --name-only --no-renames -z <base> <head>
```

so a rename/move is represented as deletion of the old path plus addition of
the new path. This preserves both semantic areas during classification. Local
pre-commit already uses `--no-renames` for the staged diff.

The trusted selector also exposes a deterministic `plan` command whose output
contains stable task IDs plus selection reasons. These IDs are the authority
boundary that Phase 3 will bind to moon tasks; candidate moon configuration is
not read while calculating the required set. A candidate change to moon/task
configuration itself selects the conservative full current task set.

A policy change cannot use the candidate policy to reduce its own required
verification. The base classifier remains authoritative, while candidate policy
self-tests are additive evidence. Once a policy revision lands on `main`, its
elevated/fail-closed rules become authoritative for later pull requests.

## Implementation roadmap

This checklist is the operational roadmap for this CI work. Update it as work is
completed or new closure requirements are discovered. A checked box means the
item has been implemented and independently verified, not merely started.

The roadmap deliberately separates SearchKernel-specific verification authority
from commodity execution infrastructure. SearchKernel owns policy, trust,
exact-head evidence, and closure. moonrepo, cargo-nextest, and sccache own the
generic orchestration and build/test acceleration they already solve.

### Phase 1: inventory and closure graph

- [x] Inventory every executable test/verifier entry point in the repository.
- [x] Record the stable profile ID and exact command for each verification unit.
- [x] Record semantic/path triggers for each verification unit.
- [x] Record profile dependencies and build an explicit verification DAG.
- [x] Determine which umbrella/cumulative verifiers genuinely subsume others.
- [x] Identify duplicated, stale, unreachable, or misleading verifier entry points.
- [x] Record native/Vespa/oracle/toolchain prerequisites for every relevant unit.
- [x] Record runner suitability: GitHub-hosted versus self-hosted.
- [x] Record mutable state, cache behavior, concurrency hazards, and isolation needs.
- [x] Record approximate runtime and useful parallelization boundaries.
- [x] Record PR-CI versus main/nightly suitability.
- [x] Record whether verifier implementation must be trusted from base or candidate.
- [x] Reconcile the Vespa milestone/integration chain, including M13/M14 coverage.
- [x] Document the resulting inventory and DAG in this file.

### Phase 2: trusted fail-closed policy

This remains SearchKernel-owned. Do not delegate verification sufficiency to
moonrepo or candidate-controlled task configuration.

- [x] Define the explicit set of non-behavioral paths that may safely select no behavioral tests.
- [x] Make unknown source/build/tooling/behavioral paths select a conservative/full task set.
- [x] Add semantic fan-out rules for schema/protocol/client changes.
- [x] Add semantic fan-out rules for ABI/native changes.
- [x] Add semantic fan-out rules for persistence/storage changes.
- [x] Add semantic fan-out rules for dependency/build-system/generated-artifact changes.
- [x] Ensure renames/moves account for both old and new semantic areas.
- [x] Make verification-policy/orchestration changes automatically select elevated verification.
- [x] Add adversarial classifier tests for new, unknown, renamed, and moved paths.
- [x] Verify the authoritative CI selector continues to come from the trusted base revision.
- [x] Make the trusted plan output stable task IDs plus human-readable selection reasons.
- [x] Ensure candidate moon/task configuration cannot reduce the trusted task set required for that candidate.

### Phase 3: adopt the commodity execution platform

Use standard tools for generic orchestration rather than building a
SearchKernel-specific scheduler/cache/test-distribution system.

#### moonrepo

- [ ] Introduce moonrepo with a pinned version/toolchain identity.
- [ ] Represent every surviving verification profile as one canonical moon task.
- [ ] Encode task dependencies from the verified Phase 1 closure graph.
- [ ] Keep repository verifier scripts as task commands where they remain the authoritative acceptance entry point.
- [ ] Add a mechanical check that every task ID emitted by the trusted selector exists in the moon graph.
- [ ] Add a mechanical check that every required full-suite verifier is reachable from the full-suite root.
- [ ] Make pre-commit and developer tooling invoke the same canonical tasks rather than duplicating commands.
- [ ] Make the developer/agent `test-plan` interface print trusted task IDs and reasons.
- [ ] Make the developer/agent `test-affected` interface execute the trusted task set through moon.
- [ ] Ensure arbitrary issue/PR shell text can add evidence but never becomes executable CI authority.

#### cargo-nextest

- [ ] Adopt cargo-nextest for Rust test tasks where it preserves the existing test contract.
- [ ] Preserve ordinary `cargo test` only where a verifier specifically depends on cargo-test behavior.
- [ ] Define nextest profiles/filtersets for the Rust task families identified in Phase 1.
- [ ] Use nextest partitions for useful Rust test sharding.
- [ ] Evaluate/use nextest archives where one compiled test corpus should execute on several workers.
- [ ] Verify sharding/archive execution preserves the same required test closure as unsharded execution.

#### sccache

- [ ] Configure sccache for rustc on CI runners.
- [ ] Configure sccache for supported C/C++ compiler invocations where practical.
- [ ] Provide a persistent or remote cache backend appropriate to the runner topology.
- [ ] Include compiler/toolchain/native-environment identity in cache correctness assumptions and provenance checks.
- [ ] Verify cache corruption/mis-keying cannot turn a required compile/test failure into a false green.
- [ ] Measure hit rates and make cache misses observable.
- [ ] Ensure a cache miss never authorizes an implicit full Vespa source rebuild in ordinary CI.

### Phase 4: provision native runners

The initial goal is a boring, reproducible immutable runner environment. Do not
block native CI on adopting Nix.

- [ ] Define the complete native runner prerequisite manifest.
- [ ] Provide a one-command/bootstrap deployment bundle for a fresh supported self-hosted runner.
- [ ] Provision the immutable SearchKernel native base/SDK and runtime closure.
- [ ] Provision a clean Vespa checkout at the exact pinned revision required by current verifiers.
- [ ] Provision required prebuilt oracle artifacts.
- [ ] Provision required compiler/toolchain/system dependencies.
- [ ] Provision pinned moonrepo, cargo-nextest, and sccache tooling.
- [ ] Give provisioned state an immutable/versioned identity and manifest/checksum validation.
- [ ] Add a cheap CI environment/provenance verifier.
- [ ] Make missing/stale prerequisites fail instead of silently rebuilding Vespa.
- [ ] Ensure shared heavyweight prerequisites are read-only/immutable.
- [ ] Ensure runtime/persistence/socket/temp state is private per CI job.
- [ ] Verify concurrent native jobs cannot contaminate each other.
- [ ] Document the bootstrap/update procedure for self-hosted runners.
- [ ] After native CI is green, evaluate Nix plus Cachix/Attic as a replacement for ad-hoc immutable prerequisite distribution.
- [ ] Treat a smaller purpose-built verifier bundle as a later optimization rather than a prerequisite for CI enablement.

### Phase 5: connect trusted policy to the moon task graph

- [ ] Extend the trusted base-owned selector to emit every required canonical moon task ID.
- [ ] Have CI execute exactly the trusted task set through moon against candidate code.
- [ ] Add GitHub Actions worker placement for non-native tasks.
- [ ] Add self-hosted worker placement for native real-engine tasks.
- [ ] Add native client/gRPC end-to-end tasks.
- [ ] Add Vespa milestone/oracle tasks.
- [ ] Keep task dependencies in the moon graph instead of duplicating them in GitHub Actions YAML.
- [ ] Run candidate policy/task-graph self-tests plus conservative/full verification when verification infrastructure changes.
- [ ] Preserve exact-head evidence for every required task.
- [ ] Detect base drift so stale evidence cannot authorize a changed merge candidate.
- [ ] Establish merge-result/current-base verification or an equivalent final gate.
- [ ] Keep post-merge `main` reconciliation/verification as part of substantive closure.

### Phase 6: authoritative full DAG

- [ ] Define one canonical full-suite moon root from the verified Phase 1 inventory/DAG.
- [ ] Mechanically ensure every surviving required verifier is reachable from that root.
- [ ] Make removing/renaming a verifier fail if the full-suite graph/inventory is not updated.
- [ ] Run the authoritative full DAG on `main`.
- [ ] Run the appropriate comprehensive DAG on nightly CI.
- [ ] Verify the full DAG includes the latest intended Vespa milestone coverage rather than relying on ambiguous umbrella names.
- [ ] Explicitly disposition retained/retired historical gates such as Phase 9 and C2 rather than silently dropping them.
- [ ] Verify the full DAG never rebuilds heavyweight immutable dependencies as a hidden fallback.

### Phase 7: optimize and distribute without weakening

Use the adopted tools before inventing custom acceleration.

- [ ] Measure per-task CI runtime, queue time, compile time, and cache hit rate after correctness coverage is complete.
- [ ] Remove only proven duplicate verification.
- [ ] Let moon parallelize independent DAG nodes.
- [ ] Use nextest partitions for Rust execution parallelism.
- [ ] Use nextest archives where build-once/test-many materially reduces repeated compilation.
- [ ] Reuse moon task caches where task inputs/outputs are safe and deterministic.
- [ ] Reuse sccache for compilation outputs.
- [ ] Keep mutable test state isolated despite cache and artifact reuse.
- [ ] Evaluate a moon-compatible remote task cache if local/self-hosted caching is insufficient.
- [ ] Evaluate Nix/Cachix/Attic for immutable native prerequisite distribution only after measured need and a working baseline.
- [ ] Add regression checks ensuring optimization does not change required verification closure.

### Phase 8: adversarial CI-system closure

- [ ] Re-run the complete false-green test matrix against the trusted selector, moon graph, nextest sharding/archive path, caches, and native provisioning.
- [ ] Verify a candidate cannot weaken its own selector, moon task definition, executor mapping, or full-suite reachability and then authorize itself.
- [ ] Verify exact-head and base-drift protections against force-push/rebase/base-change cases.
- [ ] Verify cache hits and restored artifacts cannot substitute evidence from a different semantic/toolchain/native identity.
- [ ] Verify concurrent native jobs cannot share mutable persistence/runtime state.
- [ ] Verify missing native prerequisites fail rather than triggering an expensive hidden rebuild.
- [ ] Reconcile documentation, GitHub Actions, developer commands, agent guidance, and CTO state after rollout.

## False-green properties to test

The verification system itself needs regression tests proving at least:

- a PR cannot weaken its own selector and thereby skip required tests;
- a PR cannot weaken its own executor and thereby authorize itself;
- unknown behavioral paths fail closed;
- renames/moves retain the requirements of both relevant semantic areas;
- build/dependency changes fan out to affected profiles;
- adding a new verification profile without a CI executor fails;
- removing or renaming a verifier cannot silently remove it from `test-all`;
- candidate-head evidence cannot authorize a different head;
- stale base/merge-result evidence is not treated as current evidence;
- native provisioning with the wrong pin/ABI/manifest is rejected;
- missing native prerequisites fail rather than trigger an expensive hidden
  rebuild;
- concurrent jobs cannot share mutable persistence/runtime state;
- candidate changes to verifiers/fixtures/policy trigger elevated verification.

## Definition of success

The design is complete when:

1. every surviving repository verifier/test family is represented in the
   verified inventory and canonical moon task graph;
2. PR CI independently and fail-closed selects all required task IDs from
   trusted base policy, while candidate-controlled moon configuration cannot
   reduce that mandatory set;
3. all selected tasks, including real native/Vespa paths, execute in CI against
   the exact candidate head;
4. one mechanically checked full-suite moon root reaches the complete intended
   verifier closure;
5. Rust test execution uses cargo-nextest where appropriate, including
   partition/archive reuse when it materially reduces duplicate compilation;
6. Rust/C/C++ compilation uses sccache where appropriate without weakening
   provenance or correctness;
7. coding agents can obtain a precise required-test plan without being trusted
   to decide sufficiency;
8. main/nightly CI can run the complete suite without rebuilding heavyweight
   immutable dependencies; and
9. the selector/task-graph/executor/cache/provisioning system has adversarial
   regression tests against the false-green cases above.
