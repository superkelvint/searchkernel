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

Issue acceptance requirements should prefer repository-owned profile IDs or
clear acceptance behavior rather than arbitrary shell supplied by issue text.
Arbitrary agent-reported commands remain evidence only.

## CI execution model

A planner job computes required profiles from the trusted base policy and the
candidate diff. Independent jobs execute the selected profiles in parallel.

Conceptually:

```text
trusted base policy
       |
       v
base..candidate diff
       |
       v
verification plan
       |
       +---- rust
       +---- protocol/clients/contracts
       +---- persistence
       +---- ABI
       +---- native/oracle/E2E
       |
       v
exact-head/required-evidence gate
```

Verification runs against candidate code even though authoritative selection is
made by trusted base policy.

CI should continue to guard exact-head evidence. Where practical, final
acceptance should also protect against base drift by testing the current merge
candidate/merge result or requiring re-verification after the base changes.
Post-merge `main` verification remains part of closure for substantive work.

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

## Incremental rollout

### Phase 1: inventory and closure graph

Enumerate the existing test/verifier surface. Establish which commands are
cumulative, duplicated, stale, missing from CI, or require unavailable runner
state. Build the explicit verification DAG.

### Phase 2: fail-closed policy

Harden the current exact-head classifier so unknown behavioral paths fail
closed. Add semantic fan-out/dependency rules and tests that demonstrate newly
introduced or moved paths cannot silently escape verification.

### Phase 3: canonical profiles

Move command ownership into repository-defined verification profiles. Make
pre-commit and developer commands consume the same profile definitions while
retaining base-owned authority in CI.

### Phase 4: provision native runners

Provision immutable native/Vespa/oracle prerequisites onto the self-hosted
Actions runners and add a cheap environment/provenance check. Do not permit
ordinary CI to fall back to rebuilding Vespa.

### Phase 5: complete change-aware CI

Teach the planner to expose every required profile and add CI executors for all
of them, including native and end-to-end profiles. Ensure policy changes receive
candidate self-tests plus conservative/full verification.

### Phase 6: authoritative full suite

Add `test-all` (or equivalent) from the verified inventory/DAG and run it on
`main` and/or nightly as appropriate. The full suite must be explicit rather
than an alias whose transitive coverage is assumed.

### Phase 7: optimize without weakening

Only after coverage is mechanically established, reduce duplicated work,
parallelize independent profiles, and improve caches. Optimization must preserve
the same required verification closure.

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
   inventory and verification DAG;
2. PR CI independently and fail-closed selects all required profiles from a
   trusted policy;
3. all selected profiles, including real native/Vespa paths, can execute in CI;
4. `test-all` mechanically runs the complete intended suite;
5. coding agents can obtain a precise required-test plan without being trusted
   to decide sufficiency;
6. main/nightly CI can run the complete suite without rebuilding heavyweight
   immutable dependencies; and
7. the selector/executor/provisioning system has adversarial regression tests
   against the false-green cases above.
