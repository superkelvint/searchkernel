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
- the relevant contract/specification under `reference/` or `docs/` for the
  task.

Treat repository documentation and executable contracts as the source of truth.
Do not duplicate large architecture or API specifications in this file.

Follow more-specific `AGENTS.md` files when present in a subtree.

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

1. Determine whether the current checkout is already an isolated worktree for
   this task.
2. Inspect `git status --short` and the current branch.
3. Identify pre-existing changes and treat them as belonging to someone else
   unless the task explicitly says otherwise.
4. Read the relevant implementation, tests, contract, and existing patterns.
5. For Vespa behavior, inspect the pinned source under `upstream/vespa/`; do not
   guess from API names or high-level documentation.
6. Determine how the finished change will be verified.

For a small, obvious change, implement directly.

For a multi-file, architectural, unfamiliar, or ambiguous change, explore first
and form a short implementation plan before editing.

Do not broaden the task with unrelated cleanup.

## Repository command surface

Use the root `./dev` command as the normal entry point for repository setup,
verification, generation, and acceptance lanes:

```text
./dev doctor
./dev check-fast
./dev test ir
./dev test rust-api
./dev test python
./dev regen protocol
./dev verify m13
./dev verify integration
./dev worktree create my-task
```

Start with `./dev doctor` when entering an unfamiliar checkout or when a build
fails for an environment-related reason. It reports each prerequisite
separately; fix or report failed prerequisites before diagnosing product code.
Use `./dev --help` for the supported command list. Keep direct underlying
scripts for focused debugging or when a contract explicitly names one.

For isolated implementation work, use `./dev worktree create <task-name>`.
It creates a sibling worktree at `../<repository>-<task-name>` on the
`agent/<task-name>` branch from the current `HEAD`; it does not copy
uncommitted changes from the current checkout.

## Search scope

Repository-wide discovery should exclude generated/build artifacts by default.
The canonical exclusions are defined in `.rgignore`.

In particular, do not search generated FlatBuffers/client output, BFBS files,
native SDK artifacts, build output, or Python caches during normal code
discovery.

Search excluded paths explicitly only when debugging generation, protocol
encoding, generated bindings, packaging, or SDK/native ABI output.

## Worktree isolation

Substantial implementation work should run in a dedicated Git worktree and task
branch when parallel development is possible.

The normal model is:

```text
task
    -> dedicated branch/worktree
    -> implementation
    -> focused verification
    -> task-owned commit(s)
    -> serialized self-integration
    -> combined verification
```

If the current checkout is already a task-specific worktree, use it.

Otherwise, when isolation is useful and repository permissions allow it, create
a dedicated worktree from the current integration baseline before editing.

Use short descriptive branch names consistent with repository history, for
example:

```text
agent/fix-grouping-results
agent/feat-document-update
agent/refactor-search-resolution
```

The primary checkout should be treated as an integration workspace during
parallel development, not as a shared implementation workspace.

Small changes may be performed directly when no concurrent work exists and a
separate worktree would provide no useful isolation.

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

For a reported defect, add an executable regression test that demonstrates the
failure before the fix when practical.

Regression tests must exercise observable behavior or the real implementation
path. Source-text searches, naming assertions, and similar textual proxies do
not prove runtime behavior.

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

`core/native/sdk/` is generated. Rebuild it only through repository tooling when the task actually requires a native adapter/SDK change.

Do not hand-edit generated SDK contents.

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

For Rust changes, normally run the affected crate's:

```text
cargo fmt --check
cargo clippy -p <crate> --all-targets --all-features -- -D warnings
cargo test -p <crate> --all-targets --all-features
```

Expand verification when the change has broader impact.

Run full workspace checks when the change crosses crate/API/IR/native boundaries, changes shared infrastructure, or otherwise has broad consequences:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

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
Do not claim it passed.

## Commit discipline

Completed implementation work must be committed unless the user explicitly asks
not to commit.

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

Do not push unless explicitly requested.

## Serialized self-integration

When work was performed on a dedicated task branch/worktree, the implementation
agent normally integrates its own completed work.

A separate integration agent is not required for ordinary conflict-free work.

Integration is serialized even though implementation may be parallel.

Only one worker may update the integration branch at a time.

Use the repository-provided integration command if one exists.

Otherwise use an exclusive repository-wide lock under the common Git directory,
for example with `flock`, so cooperating agents cannot race while updating the
integration branch.

Conceptually:

```text
acquire integration lock

    identify current integration HEAD
    update task branch onto that baseline
    resolve safe mechanical conflicts if necessary
    run relevant verification on the updated task branch
    fast-forward/integrate into the integration worktree
    run verification required for the combined state
    confirm integration succeeded

release integration lock
```

Do not hold the integration lock while doing ordinary implementation work.

The lock protects only the short integration phase.

### Integration rules

Before integration:

- the implementation must already be committed;
- the task worktree must contain no accidental uncommitted changes;
- the integration worktree must not contain unexplained dirty changes;
- identify the current integration branch rather than assuming an outdated
  baseline.

While integrating:

- preserve all already-integrated work;
- never resolve a conflict by discarding another task's change;
- never weaken tests or contracts to make branches combine;
- rerun relevant verification after updating against the latest integration
  branch;
- prefer fast-forward integration after updating the task branch when practical;
- preserve coherent task commits unless repository convention says otherwise.

Straightforward mechanical conflicts may be resolved by the worker when the
intended combined behavior is clear.

Examples include:

```text
independent import additions
non-overlapping manifest entries
mechanical generated-file updates
adjacent documentation edits
```

If integration exposes an actual semantic conflict, stop rather than guess.

Examples include:

```text
two incompatible public API changes
different schema meanings
conflicting IR semantics
different persistence behavior
incompatible architectural decisions
tests whose correct expectation depends on choosing one task over another
```

Report that conflict clearly for review.

After successful integration:

- confirm the integration branch contains the task commit(s);
- run `git status --short`;
- remove the task worktree/branch when safe and no longer needed;
- do not leave an abandoned integration/rebase operation behind.

## No manual merge dependency

Routine completed work should not require a human to perform Git integration.

The normal expectation is:

```text
agent implements
agent verifies
agent commits
agent acquires integration lock
agent updates against latest integrated state
agent verifies again
agent integrates
```

Human input is reserved for genuine semantic conflicts, failed requirements, or
decisions the repository does not already answer.

## Definition of done

A task that changes code is complete only when:

```text
implementation matches the requested scope
relevant contracts are preserved
focused verification passes
required acceptance verifier passes
broader verification is run when warranted
final diff has been reviewed
task-owned work is committed
task is integrated when using the normal self-integration workflow
combined state remains healthy
```

Passing tests is not sufficient if the implementation violates the required
native path, public API contract, semantic boundary, or repository rules.

## Completion report

When finished, report concisely:

- what changed;
- important verification commands and results;
- commit hash and subject;
- whether the task was integrated;
- any remaining blocker or unsupported capability;
- any pre-existing dirty state that remains relevant.

Do not claim the repository is clean unless it actually is.

Do not declare completion when required verification failed.
