# CI and verification architecture

Status: current build-system direction.

This document describes the CI/verification model Search Kernel is converging on.
The detailed design authority is
[`implementation-plan/build-system.md`](../implementation-plan/build-system.md),
and implementation sequencing is owned by
[`implementation-plan/build-system-roadmap.md`](../implementation-plan/build-system-roadmap.md).

If this document conflicts with those files, the build-system design and roadmap
win.

## 1. Core principle

Search Kernel separates two concerns that were previously coupled:

1. **mandatory pull-request CI** — intentionally small, predictable, and fast;
2. **correctness/release verification** — focused or exhaustive checks run
   explicitly against an exact revision.

Heavy native/Vespa verification is important, but it is **not** automatically
scheduled merely because a pull request touches a native-sensitive path.

The repository must not rebuild the historical model where path-based automation
expands ordinary PR CI into a large native verifier matrix.

## 2. Mandatory pull-request CI

The mandatory PR gate is:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --no-default-features --locked
cargo check -p searchkerneld --tests --features native --locked
cargo clippy --workspace --all-targets --no-default-features --locked -- -D warnings
```

The canonical semantic owner is `./dev ci`.

The target GitHub Actions shape is therefore:

```text
checkout exact PR revision
        |
        v
    ./dev ci
```

Workflow YAML should be thin orchestration. It should not independently own:

- Cargo/compiler flags;
- toolchain selection;
- native SDK/runtime setup;
- verifier semantics;
- change classification;
- subsystem dependency rules.

`.github/workflows/rust-ci.yml` checks out the event revision and delegates the
gate to `./dev ci`. The workflow owns scheduling; the canonical launcher owns
toolchain/container setup and the command flags.

### What PR CI intentionally does not do

Mandatory PR CI does **not** automatically run:

- the cumulative M14/native verifier;
- sanitizer/stress suites;
- native real-engine E2E;
- persistence qualification;
- language package E2E;
- ABI/native full bundles;
- Universe;
- release qualification.

Those checks remain available and important. They are simply not part of the
automatic PR tax.

## 3. Canonical local verification interface

Developers and local-capable coding agents use `./dev`:

```bash
./dev ci

./dev test --list
./dev test <scope>

./dev verify --list
./dev verify <lane>

./dev galaxy
./dev universe
./dev release-check
```

The intent is:

- `ci` — cheap mandatory PR gate;
- `test <scope>` — focused product/unit/client tests;
- `verify <lane>` — focused specialist or semantic verification;
- `galaxy` — broad routine suite, equivalent to `check`, without Universe-only exhaustive lanes;
- `universe` — exhaustive repository/source qualification;
- `release-check` — exact-revision shipability and artifact qualification.

Do not reflexively run `universe` for every edit. Choose the smallest checks
that actually exercise the changed behavior, then widen verification when the
risk or acceptance criteria justify it.

## 4. Exact-SHA remote verification

Many coding agents and reviewers cannot execute native, Vespa, sanitizer, or
other specialist checks locally. The replacement for putting all of those
checks into PR CI is a first-class bounded GitHub on-demand workflow.

Conceptually:

```text
agent/reviewer identifies exact pushed SHA
                  |
                  v
dispatch Search Kernel on-demand verification
  revision  = exact immutable SHA
  operation = ci | test | verify
  target    = empty for ci; known scope/lane for test or verify
                  |
                  v
trusted GitHub control plane
                  |
                  v
checkout requested SHA
prove HEAD == requested SHA
                  |
                  v
execute exactly one canonical command
                  |
                  v
structured PASS/FAIL evidence + logs
```

The supported canonical operations are:

```text
./dev ci
./dev test <scope>
./dev verify <lane>
```

Local-capable callers may use convenience wrappers:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

Remote-only agents may dispatch the same bounded workflow through an authorized
GitHub UI/API/CLI/connector without having a Search Kernel development
environment. Connectors that cannot issue `repository_dispatch` may use an
authorized pull-request comment as a second ingress to the same verifier:

```text
/searchkernel-verify ci <exact-sha>
/searchkernel-verify test <scope> <exact-sha>
/searchkernel-verify verify <lane> <exact-sha>
```

Comment requests are accepted only on pull requests and only from GitHub actors
whose `author_association` is `OWNER`, `MEMBER`, or `COLLABORATOR`. The
comment is only a request envelope: it does not supply shell text or redefine
the verifier. The exact SHA and target still pass through the same repository
membership, target-registry, trusted-control-plane, and exact-checkout checks as
`repository_dispatch`.

### Remote evidence requirements

Every remote run must identify at least:

```text
revision
operation
target
canonical_command
result
run_id
run_url
```

Evidence for SHA A is not evidence for SHA B.

The local `--remote` handoff must fail closed when relevant local state is
dirty or when the requested commit has not been pushed, so a developer cannot
mistakenly verify stale committed code while looking at newer local changes.

## 5. Trust boundary

The requested revision must not be allowed to redefine the privileged workflow
that decides how it is executed.

The on-demand control plane should therefore come from trusted/default-branch
infrastructure while the requested SHA is checked out separately as code under
test.

The current trust policy is deliberately small. It accepts authenticated
repository dispatch and authorized pull-request comments as two request
ingresses into the same trusted verifier. Comment ingress is restricted to
`OWNER`, `MEMBER`, or `COLLABORATOR` author associations and a single-line,
bounded command grammar. Remote verification must:

- accept only bounded `ci`, `test`, and `verify` operations;
- validate targets against the canonical repository-owned target registry;
- expose no arbitrary shell input;
- use read-only repository permissions by default;
- receive no release/deploy/write secrets;
- accept only exact commit SHAs available from this repository; fork-only or
  unavailable SHAs fail closed;
- use job-private writable state;
- preserve exact-SHA identity in results.

The workflow has read-only GitHub repository permissions, exposes no release,
deployment, or write secrets to subject execution, and accepts no arbitrary
shell command. Repository-dispatch callers must have dispatch access; comment
callers must be authorized collaborators by GitHub author association. There is
no separate actor allowlist or runner-permission service.

A candidate may change implementation or verifier code, but it must not gain
the ability to alter privileged orchestration for its own execution.

## 6. Verification selection is human/reviewer-directed, not PR-path automation

The previous CI design attempted to derive an authoritative verification plan
from the base-to-head diff using repository-owned path classification and
dependency policy.

That is no longer the PR-CI architecture.

The current design deliberately prefers:

1. a small invariant automatic PR gate;
2. a finite, discoverable catalog of focused tests/verifiers;
3. lightweight documentation mapping major change areas to useful checks;
4. explicit reviewer/agent choice of additional verification;
5. exact-SHA evidence for every specialist run.

This avoids rebuilding a second dependency-analysis framework whose job is to
decide whether every PR needs a heavy matrix.

A path-to-check mapping may guide humans and agents. It must not become a hidden
automatic policy engine that reintroduces `native-full` or Universe as mandatory
PR CI.

## 7. Native and Vespa verification

Real-engine verification remains a first-class correctness tool.

Examples of specialist lanes include semantic verifiers for areas such as:

```text
vespa.linguistics
vespa.text-indexing
vespa.persistence.reopen
vespa.schema-config
vespa.query-compilation
vespa.results
vespa.grouping.recursive
vespa.grouping.yql
vespa.ranking
vespa.search-reply
vespa.recovery
vespa.hardening.core
vespa.hardening.sanitizers
milestone.m14
product.integration
```

The exact supported vocabulary is owned by the canonical target registry and
must be discoverable through `./dev verify --list` and through GitHub-visible
repository state for remote-only agents.

Native verification should use the same canonical environment/runtime identities
as local build and CI tooling. Ordinary Search Kernel changes must not rebuild
Vespa from source as part of normal verification.

Immutable toolchain/Vespa inputs may be shared by identity. Writable build,
index, socket, sanitizer, persistence, and target state must remain isolated per
job/worktree as defined by the build-system plan.

## 8. ABI changes

ABI correctness is an invariant of an ABI-changing revision, not a reason to run
a permanent automatic native-full PR matrix.

When the native ABI changes:

- every checked-in ABI authority must advance together;
- `tools/check-native-abi-authorities.py` and its regression tests must pass;
- generated bindings/consumer artifacts must remain synchronized;
- the relevant ABI/native verifier should be run against the exact changed SHA;
- that evidence belongs to the PR/change that introduces the ABI bump.

Version-specific historical reconciliation issues should not be carried forward
as evergreen implementation tasks once the generic invariant is enforced.

## 9. Universe and release qualification

`./dev universe` and `./dev release-check` serve different purposes.

### `./dev universe`

Universe is exhaustive **repository/source qualification**. It may include
tests, specialist semantic verifiers, clients, hardening lanes, and historical
closures.

Universe is orchestration only: it should call canonical `./dev test` /
`./dev verify` operations rather than inventing another set of semantics.

### `./dev release-check`

Release check is **shipability qualification for one exact revision**.

It must eventually:

- consume or run the required exact-SHA source qualification;
- build the actual release artifacts;
- qualify every supported row in `docs/release-support.json`;
- prove package/runtime identity and provenance;
- verify external consumer/package behavior;
- ensure publication promotes the exact artifacts that were qualified.

A green PR gate is not release evidence. A green Universe alone is also not
proof that the distributable artifacts are shippable.

## 10. CI/container state

The build-system design retains three independent image/input identities:

```text
searchkernel-ci-toolchain:<toolchain-id>
searchkernel-ci-vespa:<vespa-commit>-<bundle-id>
searchkernel-ci:<searchkernel-revision>
```

Hard invariants:

- ordinary Search Kernel source changes do not rebuild or republish Vespa;
- the thin leaf records exact immutable toolchain/Vespa identities;
- missing required immutable inputs fail closed rather than silently using host
  state;
- local and remote canonical verification resolve the same identities;
- writable Cargo/native state is worktree- or job-private;
- caches are performance aids, never correctness authorities.

The supported normal host boundary should remain small: Git, a
Docker/container runtime, repository authentication, and normal
shell/filesystem/worktree support. The canonical environment owns the language,
compiler, FlatBuffers, and native build dependencies.

## 11. Reviewer guidance

For a substantive change, reviewers should answer:

1. What behavior changed?
2. What is the smallest focused test/verifier that can falsify it?
3. Does the change cross a high-risk boundary such as ABI, persistence,
   native memory safety, result projection, IR-to-Vespa lowering, packaging, or
   runtime ownership?
4. What additional exact-SHA specialist evidence is appropriate?
5. Does this need source qualification only, or actual release-artifact
   qualification?

Reviewers may request stronger verification at any time. That does not make the
requested lane a new automatic PR requirement for every future change.

## 12. Explicit non-goals

Do not use CI work to recreate:

- automatic native-sensitive PR classification;
- a mandatory `native-full` PR bundle;
- a generic verification scheduler/DSL;
- a new dependency-analysis framework;
- a generalized remote shell;
- mandatory Universe on ordinary pull requests;
- ordinary per-commit Vespa rebuilds;
- duplicated verifier semantics in GitHub Actions YAML.

## 13. Migration notes

Some historical issues may still describe the older change-aware PR verification
architecture.

When encountered, reconcile them against the current build-system design rather
than implementing them literally.

In particular:

- stale issues that require automatic heavy/native PR verification should be
  reassessed;
- obsolete path-selection machinery should be removed rather than revived;
- specialist verifiers should migrate behind canonical `./dev verify` targets;
- remote execution should become first-class before obsolete heavy CI paths are
  deleted if those paths are still the only way an agent can run a required
  check.

## 14. Definition of success

The CI/verification architecture is converged when:

- automatic PR CI delegates to `./dev ci`;
- the mandatory PR gate remains intentionally thin;
- all important specialist checks remain available through canonical
  `./dev test` / `./dev verify` targets;
- remote-only agents can discover and dispatch those targets against an exact
  SHA;
- remote results carry exact-revision structured evidence;
- the remote control plane is trusted and bounded;
- heavy/native verification is not automatically inferred from PR paths;
- local and remote execution use the same canonical environment semantics;
- `universe` provides exhaustive source qualification;
- `release-check` qualifies the actual distributable artifacts;
- stale CI documentation/issues no longer reintroduce the retired automatic
  heavy-PR model.
