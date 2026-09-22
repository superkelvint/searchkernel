# AGENTS.md

## Project

Search Kernel is a thin Rust interface to Vespa's local C++ search engine.

Search Kernel owns the product API, portable semantics, Rust/native boundary,
integration, packaging, and fidelity tests.

Vespa owns the search engine: persistence, indexing, attributes, matching,
query execution, ranking, tensors, vectors, concurrency, and low-level resource
management unless a task explicitly says otherwise.

When choosing between faithfully exposing Vespa and inventing a more general
Search Kernel subsystem, prefer Vespa.

## Read first

Before making a non-trivial change, read:

- `docs/reading-the-codebase.md` for repository navigation;
- `docs/architecture.md` for current architecture and dependency boundaries;
- `BUILDING.md` for build/compile commands, toolchain and native SDK setup,
  verification commands, and environment troubleshooting;
- the relevant contract/specification under `reference/` or `docs/` for the
  task.

Treat repository documentation and executable contracts as the source of truth.
Do not duplicate large architecture or API specifications in this file.

Follow more-specific `AGENTS.md` files when present in a subtree.

## GitHub work coordination

GitHub is the authoritative CTO work queue and review channel for this
repository.

Use `superkelvint/searchkernel` issues and pull requests for all implementation
coordination:

- GitHub Issues hold bugs, audit findings, remediation work, feature requests,
  and other actionable CTO work.
- Do not use a separate `dispatches/` directory, `agent-reports/` directory,
  or coordination repository as the work queue or completion channel.
- Do not search GitHub merely because a task is non-trivial. If the user gives a
  concrete implementation task directly, treat that task as the work
  specification; do not search or enumerate the wider issue queue first.
- For a directly assigned non-trivial implementation task that is not already
  tied to an issue, create a new GitHub issue for that exact task before code
  changes begin. The issue is for durable tracking and PR linkage, not for
  rediscovering or redefining the user's task.
- **Create every new actionable implementation issue with its canonical queue
  labels in the same creation operation.** At minimum this means exactly one
  workflow-state label plus priority, area, and type classification. Ordinary
  unclaimed work MUST be created with `status:ready`; do not create an
  unlabeled issue and plan to classify it later.
- The canonical issue taxonomy is defined by
  `searchkernel-cto-state/ISSUE_LABELS.md`. For ordinary actionable work use:
  `status:ready`, exactly one `priority:p0`..`priority:p3`, at least one
  stable `area:*`, and one primary `type:*` where practical. If the issue is
  genuinely not actionable at creation time, use `status:blocked` instead.
  Follow-up work uses `status:needs-followup`. Tracking/umbrella issues are not
  implementation candidates and MUST NOT be labeled `status:ready`.
- If a newly-created issue will be claimed immediately, it is still created as
  `status:ready` first; the race-safe claim operation then transitions it to
  `status:in-progress`. Issue creation itself is not a claim.
- Search or browse the issue queue only when the user asks to find/select/claim
  work, explicitly invokes an issue-discovery workflow, or the task itself
  requires broader issue context.
- If the user names an issue or pull request, fetch that item directly. Do not
  enumerate or search the wider GitHub queue first.
- For issue-driven work, read the full issue and recent comments, check for
  linked/open pull requests, and make sure another agent has not already
  claimed it.
- Claim work before editing by assigning yourself when permissions allow and by
  leaving an issue comment that clearly says the issue is claimed and names the
  task branch. If an issue is already claimed, do not duplicate the work unless
  the issue explicitly calls for parallel investigation.
- Keep durable status, blockers, and scope changes on the issue or pull request,
  not in ad-hoc report files.
- Implementation happens on a task branch and is delivered through a pull
  request into `main`.
- Every implementation pull request must reference its issue. Use
  `Closes #<issue>` only when merging the pull request will fully resolve the
  issue; otherwise use a non-closing reference and leave the issue open.
- The pull request is the implementation report. Its description and comments
  must contain the evidence needed to review the work: scope, important design
  decisions, verification commands/results, and any remaining limitations.

The implementation agent may claim that a pull request is ready for review. It
does not independently declare its own work VERIFIED or CLOSED. Final review
and merge provide that disposition.

### Definition of done for substantive work

Implementation complete is **not** closure.

For substantive changes, especially lifecycle/concurrency, portable semantics,
schema conversion, persistence/reopen, result encoding, native boundaries, and
other high-blast-radius work, the reviewer/CTO must perform all of the following
before treating the work as done:

1. independently adversarially review the implementation and try to falsify it;
2. verify the applicable checks against the exact current head being accepted;
3. reconcile the implementation against the issue acceptance criteria, frozen
   contracts, and architectural invariants;
4. confirm merged/closed work is actually reachable from `main`;
5. reconcile issue/PR state after merge, closing stale or superseded blockers
   only when the current tree proves their acceptance conditions;
6. update project control-plane state when the active frontier materially
   changes.

A green happy-path test suite is evidence, not closure. Adversarial review must
ask how the change could be falsely green: races/interleavings, malformed input,
absence-vs-empty state, persistence/reopen, alternate ingress paths, generated
artifact drift, error-domain mismatch, architectural bypasses, silent fallback,
or stale/unmerged work as applicable.

Scale review depth to blast radius. Trivial documentation or mechanical edits do
not require a heavyweight audit; correctness-sensitive changes do.

Work in architectural closure clusters where appropriate. The invariant being
closed is the unit of reasoning; individual issues and pull requests are bounded
review units within that cluster.


### Self-unblock assigned work

When the user directly assigns an issue or task, optimize for forward progress
toward that outcome. Do not treat an unmet repository-internal dependency as a
reason to stop when the dependency itself is actionable.

If the assigned issue depends on other issues or prerequisite changes:

- inspect the current state of each prerequisite before declaring a blocker;
- if a prerequisite is open, unclaimed (or otherwise safely claimable), and can
  be completed in the current repository/runtime, treat it as the next unit of
  work;
- walk the dependency chain to the nearest actionable leaf, claim and complete
  that atomic issue, hand it off through its own pull request, then continue
  toward the originally assigned issue without requiring the user to prompt
  again;
- preserve one-issue/one-PR review boundaries while doing this; self-unblocking
  does not mean combining the dependency chain into one oversized change;
- treat work on an explicitly required prerequisite as part of completing the
  assigned outcome, not as unrelated scope expansion;
- re-evaluate stale blocker comments against current GitHub state instead of
  assuming they are still true;
- for audit/final-verification issues whose gate requires remediation issues to
  land first, advance the missing remediation issues rather than merely
  reporting that the audit cannot start yet.

An open prerequisite, unmet dependency gate, missing generated artifact, build
prerequisite, or failing repository check is **not by itself a terminal
blocker**. First attempt the repository-documented remediation and any
actionable prerequisite work that is within the agent's permissions and scope.

Stop and report a blocker only when making further progress requires something
the agent cannot safely or legitimately resolve itself, such as:

- a required user/product/specification decision;
- credentials, permissions, or an external service the agent cannot obtain;
- a prerequisite currently owned by another active worker where taking it would
  duplicate or conflict with that work;
- an unavailable runtime/toolchain prerequisite after following `BUILDING.md`
  recovery guidance;
- a genuine contract conflict that requires CTO/user disposition.

When stopping, name the first non-self-resolvable blocker and the concrete
action required from someone else. Do not stop merely because another issue is
open.

### Follow-up and CTO handoff lifecycle

When the CTO/reviewer requests follow-up on an existing issue/pull request,
use GitHub labels as the durable workflow state:

```text
issue: status:needs-followup
    ↓
issue: status:in-progress
    ↓
PR: needs-cto-review
```

The transitions are authoritative:

- When review finds a concrete repair or missing verification, keep or convert
  the pull request to Draft when it is not ready, remove `needs-cto-review`
  from the **pull request** if present, add `status:needs-followup` to the issue,
  and leave actionable review feedback describing the defect, expected repair,
  and required verification.
- A follow-up worker must identify the one existing open pull request before
  claiming. To claim, remove `status:needs-followup` from the issue, add
  `status:in-progress` to the issue, and comment on the issue exactly:
  `Claimed follow-up; working on PR #X.`
- Follow-up work normally updates the existing pull request branch. Do not
  create a replacement pull request merely to perform review-requested work.
  Make only the requested correction or run the missing verification.
- A successful follow-up handoff requires a pull request comment stating what
  changed and the exact verification commands/results, plus the tested head
  SHA. Then mark the pull request **Ready for review**, remove
  `status:in-progress` from the issue, and add `needs-cto-review` to the
  **pull request**.
- Ordinary implementation work follows the same review signal: after the
  implementation PR exists and all required verification passes, add
  `needs-cto-review` to the **pull request** before handing it to the
  CTO/reviewer. Do not put that label on the issue.
- `needs-cto-review` on a pull request is the durable signal that an
  implementation/follow-up worker claims the PR is complete and is explicitly
  handing it back to the CTO/reviewer. It is not verification or acceptance by
  itself.
- If follow-up is blocked or required verification still fails, keep
  `status:in-progress` on the issue, leave the exact blocker and next step
  on the issue or pull request, do not add `needs-cto-review` to the PR, and
  do not mark the pull request ready. If the claim is discovered to be invalid
  before meaningful work begins, restore `status:needs-followup` on the issue and
  remove `status:in-progress`.
- On a PR labeled `needs-cto-review`, the CTO/reviewer independently verifies
  the current pull request head. If accepted, merge normally. If more follow-up
  is required, remove `needs-cto-review` from the PR, add
  `status:needs-followup` to the issue, and leave another actionable review request.

Follow-up workers do not approve, merge, close, or declare their own work
VERIFIED/CLOSED.

## Issue sizing and decomposition

Actionable issues must be atomic and reviewable.

The default unit of work is:

```text
one issue
    ↓
one clear outcome
    ↓
one coherent implementation
    ↓
one focused pull request
```

An actionable issue should normally:

- describe one bug, one narrowly scoped feature, one refactor, or one verifier
  repair;
- have a bounded acceptance surface that a reviewer can understand without
  reconstructing a project plan;
- be small enough that its pull request can be reviewed as one coherent change;
- avoid bundling independent fixes merely because they touch the same subsystem.

If an issue contains independently reviewable outcomes, split it into separate
issues **before implementation begins**. Examples that normally require a split:

- several unrelated bugs;
- a verifier repair plus a production behavior change;
- multiple semantic bypasses that can be fixed and tested independently;
- a broad milestone or subsystem cleanup containing several distinct changes;
- implementation plus unrelated cleanup or modernization.

Prefer several small sequential or parallel pull requests over one large pull
request that is difficult to reason about.

Tracking/umbrella issues are allowed for larger initiatives, but they are not
implementation tasks. A tracker must link its atomic child issues. Agents must
claim and implement the child issue, not the umbrella issue.

One pull request should normally close one actionable issue. A pull request may
close multiple issues only when they are inseparable manifestations of the same
change and reviewing them separately would create artificial or broken
intermediate states.


## Repository map

```text
searchkernel2/
├── api/                 # canonical FlatBuffers portable API
├── core/
│   ├── crates/          # Rust implementation
│   └── native/          # C/C++ Vespa adapter and generated SDK
├── clients/             # non-Rust SDKs
├── searchkerneld/       # daemon / gRPC transport host
├── searchkernel-web/    # administration dashboard
├── upstream/vespa/      # pinned Vespa checkout; read-only
├── reference/           # frozen contracts, corpora, oracles, verifiers
├── tools/               # build/generation/verification tooling
└── docs/                # architecture and development documentation

```

## Architectural invariants

These are non-negotiable unless the task explicitly changes the architecture:

- FlatBuffers is the sole canonical portable domain contract.
- `searchkernel-ir` owns portable semantics through `*_to_ir()`.
- There must be one implementation of portable semantics.
- Rust uses the engine directly in-process.
- First-party non-Rust clients use gRPC carrying canonical FlatBuffer bytes.
- Protobuf is transport framing only; it does not duplicate the domain model.
- `searchkernel-ir-adapter` is the integration boundary between portable IR and
  the Vespa-facing engine.
- Vespa types stay behind the native adapter.
- The retired portable client FFI/native-transport architecture must not return.
- Do not independently implement Vespa engine behavior in Rust merely to make a
  test pass.

Prefer the smallest correct design that preserves these invariants.

## Source of truth

For wrapper behavior, authority is:

1. frozen API/milestone contract;
2. pinned Vespa source and tests;
3. supplied direct oracle/golden behavior;
4. Search Kernel implementation.

The implementation does not define expected behavior.

If the contract and Vespa behavior appear incompatible, surface the conflict.
Do not silently redefine either one.

## Starting a task

Before editing:

1. Determine whether the task is directly assigned or selected from the issue
   queue. For a concrete task assigned directly by the user, do not search the
   wider GitHub issue queue. If the task is a non-trivial implementation and no
   issue was named, create a new issue describing that exact task before code
   changes begin. If the user named an issue, fetch it directly. Search the
   queue only when the user asked you to select/find work.
2. Read the task's issue, its comments, and linked pull requests when they
   exist. For a newly created direct-task issue, do not expand or reinterpret
   the user's scope merely to fit other GitHub work. For selected issue work,
   confirm it is an atomic actionable issue rather than an umbrella. If it
   contains multiple independently reviewable outcomes, split them into child
   issues before implementation. Confirm the issue is not already claimed, then
   claim it before changing code.
3. Start from the current `main` branch and create a dedicated task
   branch/worktree. Prefer a branch name that includes the issue number, for
   example `agent/123-fix-grouping-order`.
4. Inspect `git status --short` and the current branch in the task worktree.
5. Identify pre-existing changes and treat them as belonging to someone else
   unless the task explicitly says otherwise.
6. Read the relevant implementation, tests, contract, and existing patterns.
7. For Vespa behavior, inspect the pinned source under `upstream/vespa/`; do not
   guess from API names or high-level documentation.
8. Determine how the finished change will be verified and what evidence the
   pull request must contain.

### Reported bugs and issues: regression test first

When the task is prompted by a reported bug, defect, regression, incorrect behavior,
or other issue, **the first code change MUST be an executable regression test that
reproduces the issue.**

Required order:

1. add the smallest regression test that exercises the real affected behavior/path;
2. run that test against the current implementation and confirm that it fails for the
   reported reason;
3. only after the failing regression is demonstrated, modify production code to fix
   the issue;
4. rerun the regression test and confirm it passes;
5. run the broader verification required by the affected area.

Do not start by editing production code and add a test afterward. Do not combine the
first reproduction test with the fix in a way that prevents observing the pre-fix
failure.

If the issue cannot be reproduced with an executable test, do not guess at a fix.
Continue investigating the reproduction path or report that the issue could not be
reproduced, including the evidence and commands used.

For a small, obvious change that is not a reported bug or issue, implement directly.

For a multi-file, architectural, unfamiliar, or ambiguous change, explore first
and form a short implementation plan before editing.

Do not broaden the task with unrelated cleanup.

## Build, compile, and environment guidance

[BUILDING.md](BUILDING.md) is the authoritative guide for repository setup,
toolchains, Cargo/FlatBuffers/native SDK builds, standard build and verification
commands, and environment recovery.

Before running build, compile, or verification commands in an unfamiliar
checkout, read `BUILDING.md`. If a command fails for an environment,
toolchain, linker, native SDK, dependency, or build-provenance reason, stop
diagnosing product code and follow `BUILDING.md` first. Start with
`./dev doctor`; fix or report failed prerequisites before changing
implementation code.

Do not improvise environment repairs such as `cargo clean`, deleting target
directories, copying SDKs between environments, or rebuilding immutable SDK
cache objects unless `BUILDING.md` explicitly instructs it.

Worktree creation remains governed by **Worktree isolation** below.

## Search scope

Repository-wide discovery should exclude generated/build artifacts by default.
The canonical exclusions are defined in `.rgignore`.

In particular, do not search generated FlatBuffers/client output, BFBS files,
native SDK artifacts, build output, or Python caches during normal code
discovery.

Search excluded paths explicitly only when debugging generation, protocol
encoding, generated bindings, packaging, or SDK/native ABI output.

## Worktree isolation

Implementation agents MUST work in a dedicated task worktree.

Before modifying any tracked file:

1. Determine whether the current checkout is already a dedicated worktree for
   this task.
2. If it is not, create a task branch and worktree from the current integration
   HEAD.
3. Change into that worktree before editing.
4. Perform all implementation, verification, and commits there.

Do not modify tracked files in the primary integration checkout.

The only exception is when the user explicitly instructs you to work in the
current checkout.

If a worktree cannot be created, stop and report the reason rather than
silently falling back to the integration checkout.

**If you are not already in a task worktree, run: ./dev worktree create <task-name>**

## Worktree safety

Multiple humans or agents may operate on the repository concurrently.

- Never discard, reset, clean, overwrite, or revert changes you did not create.
- Never use destructive Git commands on another task's work.
- Do not reuse another agent's worktree.
- Do not modify another worktree merely to obtain a clean status.
- Do not use `git stash` as a way to hide someone else's work.
- Stage explicit task-owned paths or hunks.
- Avoid `git add -A` or equivalent broad staging in a dirty checkout.
- Before committing, inspect both the working diff and staged diff.
- If unrelated changes prevent verification, report that rather than modifying
  them.

## Implementation discipline

Make the smallest good change that satisfies the task and architecture.

Do not:

- invent public APIs when a frozen API exists;
- modify acceptance artifacts merely to make implementation pass;
- hard-code fixture IDs, queries, scores, documents, or expected results;
- special-case verifier input;
- introduce a fake production backend;
- patch upstream Vespa;
- hand-edit generated SDK/protocol outputs;
- add abstractions without demonstrated need;
- add production dependencies without a concrete reason;
- perform unrelated renames, formatting sweeps, or dependency churn.

Prefer existing repository patterns over new conventions.

For any reported bug, defect, regression, incorrect behavior, or issue, regression
test-first is mandatory. The regression test must be written and observed failing
**before** production code is changed. Only then proceed to the fix.

Regression tests must exercise observable behavior or the real implementation
path. Source-text searches, naming assertions, and similar textual proxies do
not prove runtime behavior. A test that passes before the fix is not a valid
regression reproduction for the reported issue.

## Leave durable breadcrumbs

When understanding or changing code requires discovering something non-obvious,
leave the repository easier for the next human or agent to understand.

If you learn a fact that would likely cause a future maintainer to repeat the
same investigation, record it in the narrowest durable place that will be found
when they encounter the code.

Good things to document include:

- why an implementation is shaped in a way that looks unnecessarily indirect;
- invariants that are not obvious from the types;
- behavior required by Vespa, a frozen contract, or an external compatibility
  boundary;
- ordering, ownership, lifetime, concurrency, or persistence assumptions;
- tempting simplifications that are incorrect;
- surprising failure modes or edge cases;
- places where two apparently equivalent paths intentionally differ;
- non-obvious verification required when changing the code;
- a failed approach when someone is reasonably likely to try it again.

Prefer, in order:

1. a short comment beside the relevant code when the knowledge is local;
2. a module/type/function doc comment when it explains an abstraction;
3. an existing architecture or development document when the knowledge applies
   across components;
4. a focused new document only when the subject genuinely needs one.

Write the **reason**, constraint, or invariant rather than narrating what the
code already says.

Good:

```rust
// Vespa evaluates this against the pre-update document. Do not move the
// condition check after DocumentUpdate construction; create-if-missing then
// acquires different semantics.
```

Bad:

```rust
// Check the condition and construct the update.
```

When useful, point to the authoritative source rather than duplicating it:

```rust
// Keep this conversion lossless. Portable defaults are resolved in
// searchkernel-ir, not here; see docs/architecture.md "Semantic structure".
```

Do not leave:

- comments about the agent's process or conversation;
- temporary implementation plans after the work is complete;
- speculative notes that were not verified;
- large duplicated explanations already maintained elsewhere;
- comments whose only purpose is to compensate for confusing code that can
  simply be made clearer.

A useful rule of thumb:

> If you had to inspect several files, read upstream Vespa, run an experiment,
> or debug a subtle failure to understand why this code must behave this way,
> consider leaving a breadcrumb for the next person.

Before completing a substantial task, ask:

```text
Did I discover anything non-obvious that the next maintainer would otherwise
have to rediscover?
```

If yes, document it as part of the change.

## Vespa and native boundary

`upstream/vespa/` is read-only.

Use the repository's authoritative version metadata/tooling to determine the
expected pinned Vespa revision. Do not silently update it.

Inspect actual pinned Vespa source and tests when wrapping a capability.

All Search Kernel-specific C/C++ adaptation belongs under:

```text
core/native/vespa/
```

Do not spread Vespa headers or types into ordinary Rust code.

The public native boundary uses Search Kernel-owned primitives and opaque
handles.

Keep raw `unsafe` in the dedicated bindings layer.

For every boundary value, ownership must be clear:

```text
who allocates
who owns
how long borrows remain valid
who frees
whether concurrent access is allowed
```

C++ exceptions must be caught inside the adapter.

Rust panics must not unwind into C++.

Prefer clear boundary copies over fragile zero-copy lifetimes until measurement shows a copy materially matters.

Vespa SDK acquisition, build provenance, cache behavior, and environment
recovery are documented in [BUILDING.md](BUILDING.md). Follow that guide
rather than duplicating or improvising native build rules here.


## Reference and verifier artifacts

Contracts, scenarios, expected outputs, oracles, comparators, and verifier
scripts under `reference/` are frozen acceptance artifacts unless the task
explicitly asks to author or repair them.

Do not weaken, rewrite, or regenerate them merely to make implementation pass.

Later milestones must preserve earlier completed milestone behavior.

If a task supplies an acceptance command, run that exact command before calling
the task complete.

A scripted or mock backend may support focused tests. It does not prove a
real-engine milestone when the contract requires Vespa.

## Rust

Write boring, readable, idiomatic Rust.

Prefer:

- one clear owner;
- borrowed access where natural;
- explicit ownership transfer;
- concrete types before traits;
- small modules with obvious responsibilities;
- `Result` for operational failure;
- `Option` for absence;
- deterministic conversion and ordering where required by the contract.

Avoid:

- unnecessary `Arc`, `Mutex`, cloning, or interior mutability;
- trait/factory/backend hierarchies without real polymorphic need;
- `unwrap()` or `expect()` for malformed input or operational failure;
- wrapper-level nondeterminism;
- broad lint suppressions;
- clever abstractions that make control flow or ownership harder to follow.

Do not optimize away simple safe code without evidence.

## Verification

Verification must produce evidence, not an assertion that the code "looks correct."

During implementation, run the narrowest useful test or verifier repeatedly.
For the standard Rust build/lint/test command matrix, native prerequisites, and
environment recovery, follow [BUILDING.md](BUILDING.md). It is authoritative
for how compilation and build failures are diagnosed.

Expand verification when the change has broader impact.

Also:

- run the exact milestone/acceptance verifier when one exists;
- verify generated artifacts when changing their source;
- use the real Vespa path when the contract requires it;
- verify persistence/reopen when persistent behavior changes;
- verify both sides of a shared semantic path when changing portable semantics.

There must be zero compiler and Clippy warnings.

Do not weaken lint settings merely to pass.

For substantial changes, perform a fresh final diff review focused on:

```text
correctness
contract compliance
architectural boundaries
regressions
error handling
unintended scope
missing tests
```

If a required check cannot run, record the exact command, failure, and reason.
For an environment-related failure, follow `BUILDING.md` before classifying
the failure as a product defect. Do not claim a check passed when it did not run.

## Commit discipline

Completed implementation work must be committed and pushed to its task branch
unless the user explicitly asks not to commit or push.

Before committing:

1. run the required focused verification;
2. run `git status --short`;
3. inspect the complete task diff;
4. stage only task-owned changes;
5. inspect `git diff --cached`;
6. commit.

Commit messages describe the code change, not the agent activity.

Before choosing a message, inspect recent history:

```text
git log -10 --oneline
```

Follow a clear repository convention when one exists.

If history has no consistent convention, use Conventional Commit form:

```text
<type>(<scope>): <imperative summary>
```

Common types:

```text
feat
fix
refactor
test
docs
perf
build
ci
chore
```

Examples:

```text
fix(grouping): preserve nested aggregation results
feat(update): execute text field assignments
refactor(ir): unify search request resolution
test(dispatch): cover malformed protocol envelopes
docs(agents): tighten agent workflow guidance
```

Keep the subject concise and imperative.

For a non-trivial commit, add a body explaining why the change was needed and
any important design constraint or tradeoff.

Prefer one coherent commit for a small task.

Use multiple commits when the pieces are independently reviewable and each
commit leaves the branch in a sensible state.

Do not amend, rebase, squash, or rewrite commits belonging to another task.

Push the task branch after verification and open or update the pull request.
Do not push implementation commits directly to `main`.

## Pull request integration

Implementation may happen in parallel. Integration happens through GitHub pull
requests.

The normal flow is:

```text
issue exists
    ↓
agent claims issue
    ↓
task branch/worktree
    ↓
regression test first when fixing a reported issue
    ↓
implementation + verification
    ↓
commit and push
    ↓
pull request into main
    ↓
review / requested fixes
    ↓
merge when accepted
    ↓
issue closes when fully resolved
```

Implementation agents must not update `main` directly and must not use the old
self-integration workflow.

### Pull request requirements

Every implementation pull request should contain:

- the linked issue number;
- a concise summary of the actual change;
- the relevant contract or architecture constraints;
- for reported bugs/issues, the regression test and evidence that it failed
  before the production fix;
- the exact verification commands run and their results;
- any required verifier/acceptance result;
- remaining limitations, unsupported cases, or follow-up work.

Prefer `Closes #<issue>` in the pull request body when the merge fully resolves
the issue. Do not close an issue merely because an agent says the code is done.

### Review and merge

The pull request is a review boundary, not a notification after integration.

Before merge:

- required focused and broader checks must pass;
- frozen contracts and architecture invariants must still hold;
- review comments must be resolved with code/tests where appropriate;
- the final PR diff must be reviewed for correctness, regressions, unintended
  scope, and missing coverage.

An implementation agent may mark the pull request ready for review and respond
to review feedback. It must not approve its own work, call its own work VERIFIED
or CLOSED, or self-merge unless the issue or pull request explicitly instructs
that agent to do so.

The CTO/reviewer owns the final disposition. Merge the pull request only when
the evidence supports it.

### Rejection and requested changes

A rejection must be actionable. Do not merely say that an implementation is
wrong, incomplete, suspicious, or not trusted.

When rejecting an issue implementation or requesting changes on its pull
request, the CTO/reviewer must state:

1. **What is wrong** — the concrete defect, missing requirement, regression,
   architectural violation, or unsupported claim.
2. **Evidence** — the failing test, diff location, runtime behavior, contract
   text, Vespa source behavior, or other evidence that demonstrates the
   problem.
3. **Expected behavior** — what the implementation must do instead.
4. **How to fix it** — a concrete remediation direction specific enough that an
   implementation agent can act without guessing the reviewer's intent.
5. **How to verify the repair** — the regression, focused test, acceptance
   command, or observable evidence required before re-review.

Good review feedback narrows the repair. It may identify likely files, semantic
boundaries, or test cases when that materially helps the agent make the correct
change.

Do not leave feedback such as:

```text
this is wrong
needs more work
I don't trust this implementation
fix the tests
```

without explaining the defect and repair path.

If the required repair is itself independently reviewable and materially larger
than the original atomic issue, create a follow-up issue rather than silently
expanding the current issue into a large project. Keep the original issue open
when its acceptance criteria are not yet satisfied.

After merge:

- confirm the intended issue was closed when the PR used a closing reference;
- leave partially resolved issues open with the remaining work stated clearly;
- remove the task worktree/branch when safe and no longer needed;
- do not leave an abandoned merge/rebase operation behind.

## Definition of done

A task that changes code has two distinct completion states.

### Ready for review

Implementation is ready for CTO/reviewer verification only when:

```text
implementation matches the requested issue scope
reported bugs/issues have a regression test demonstrated failing before the fix
relevant contracts are preserved
focused verification passes
required acceptance verifier passes
broader verification is run when warranted
final diff has been reviewed
task-owned work is committed and pushed
a pull request into main exists and contains verification evidence
```

### Closed

The issue is CLOSED only after the pull request has been reviewed, accepted, and
merged, and the issue is actually resolved.

Passing tests is not sufficient if the implementation violates the required
native path, public API contract, semantic boundary, repository rules, or review
requirements.

## Completion report

The pull request is the durable completion report. When handing work back for
review, report concisely in the PR and to the caller:

- issue number;
- pull request URL/number;
- what changed;
- important verification commands and results;
- commit hash and subject;
- whether the PR is ready for review or already merged;
- any remaining blocker or unsupported capability;
- any pre-existing dirty state that remains relevant.

Do not create a separate agent-report file for this purpose.

Do not claim the repository is clean unless it actually is.

Do not call work CLOSED before its resolving pull request is merged, and do not
declare it ready for review when required verification failed.
