# Agent workflow

This document contains the detailed task-coordination and handoff rules for
implementation agents working in `superkelvint/searchkernel`.

The root [AGENTS.md](../AGENTS.md) contains the small set of rules that should
remain in working context for ordinary code changes. Read this workflow when a
task involves issue creation or claiming, worktrees, follow-up work, pull
requests, review handoff, or completion state.

## GitHub is the work queue

GitHub Issues and pull requests are the authoritative implementation queue and
review channel.

- Issues hold bugs, audit findings, remediation work, features, and other
  actionable work.
- Do not create ad-hoc `dispatches/`, `agent-reports/`, or another repository
  as a parallel work queue.
- If the user gives a concrete task directly, treat that task as the
  specification. Do not enumerate the wider issue queue first.
- If the user names an issue or pull request, fetch that item directly. Search
  the wider queue only when asked to find/select/claim work or when broader
  queue context is necessary.
- Keep durable status, blockers, scope changes, and verification evidence on the
  issue or pull request.

For a directly assigned non-trivial implementation task that has no issue,
create an issue for that exact task before editing. Create it with canonical
queue labels in the same operation:

- exactly one workflow-state label, normally `status:ready`;
- exactly one `priority:p0` through `priority:p3`;
- at least one stable `area:*` label;
- one primary `type:*` label where practical.

Use `status:blocked` only when the work is genuinely not actionable.
Tracking/umbrella issues are not implementation candidates and must not be
marked `status:ready`.

If newly created work will be claimed immediately, create it as
`status:ready` first, then perform the claim transition. Issue creation is not
itself a claim.

The repository queue checker is:

```bash
python3 tools/check-issue-queue.py --repo superkelvint/searchkernel
```

## Claiming work

Before editing issue-driven work:

1. Read the full issue and recent comments.
2. Check for linked/open pull requests.
3. Confirm that the issue is atomic rather than an umbrella tracker.
4. Confirm another agent has not already claimed it.
5. Create a task branch/worktree from current `main`.
6. Transition the issue to `status:in-progress` and leave a claim comment
   naming the task branch.

If another worker owns the task, do not duplicate the work unless the issue
explicitly calls for parallel investigation.

## Issue sizing

The normal review unit is:

```text
one issue
    ↓
one clear outcome
    ↓
one coherent implementation
    ↓
one focused pull request
```

Split work before implementation when an issue contains independently
reviewable outcomes, such as:

- unrelated bugs;
- a verifier repair plus a production behavior change;
- independent semantic bypasses;
- a broad milestone cleanup with separable changes;
- implementation plus unrelated modernization.

Umbrella issues may track initiatives, but agents implement their atomic child
issues. One pull request should normally close one actionable issue.

## Worktree isolation and safety

Implementation work belongs on a dedicated task branch/worktree. Do not edit
tracked files in the primary integration checkout unless the user explicitly
instructs you to work there.

When a worktree is needed, use:

```bash
./dev worktree create <task-name>
```

Multiple humans or agents may work concurrently:

- never discard, reset, clean, overwrite, or revert changes you did not create;
- never use destructive Git commands on another task's work;
- do not reuse or modify another agent's worktree;
- do not use `git stash` to hide someone else's changes;
- stage explicit task-owned paths or hunks;
- avoid broad staging such as `git add -A` in a dirty checkout;
- inspect both working and staged diffs before committing;
- report unrelated state that prevents verification instead of modifying it.

## Self-unblocking assigned work

Optimize for completing the assigned outcome. An unmet repository-internal
dependency is not automatically a terminal blocker.

If an assigned task depends on other work:

- inspect the current state of the prerequisite;
- re-evaluate stale blocker comments against current GitHub state;
- if a required prerequisite is open, safely claimable, and actionable in the
  current repository/runtime, advance the nearest actionable prerequisite;
- preserve one-issue/one-PR review boundaries while walking the dependency
  chain;
- after handing off the prerequisite, continue toward the originally assigned
  outcome.

Stop only when progress requires something you cannot safely resolve, such as:

- a product/specification decision;
- unavailable credentials or permissions;
- a prerequisite actively owned by another worker;
- an unavailable environment prerequisite after following `BUILDING.md`;
- a genuine contract conflict requiring reviewer/CTO disposition.

When blocked, name the first non-self-resolvable blocker and the concrete
external action required.

## Exact-SHA remote verification

Remote verification is a first-class path for agents or reviewers that cannot
run a required Search Kernel check locally. The semantic authority remains
`./dev`; GitHub only provides bounded orchestration around an exact pushed
commit.

The finite public target vocabulary is `build/targets.json`. Use that same
registry for both local and remote target selection. Do not invent a workflow-
only verifier name or shell command.

### Local-capable agent

When a checkout exists but a required check cannot run locally:

```bash
./dev test --remote <scope>
./dev verify --remote <lane>
```

The wrapper must hand off the exact current `HEAD`, not a branch name. It
refuses a dirty worktree/index or a commit that is not available from GitHub,
dispatches the bounded on-demand workflow, waits for the exact correlated run,
surfaces its run URL and failed-step logs, and propagates the run result.

Do not bypass the dirty-state refusal merely to obtain a green remote run. Commit
and push the source that is actually being reviewed, then rerun the required
remote checks.

### Remote-only agent

No local Search Kernel environment is required. A remote-only agent may use
either of the bounded GitHub ingresses below. In both cases, first inspect
`build/targets.json` at the revision under test and identify the exact pushed
40-character commit SHA.

#### Pull-request comment ingress

This is the normal path for connector-only agents that can read/write GitHub
pull-request comments but cannot issue `repository_dispatch`.

Post a new top-level comment on the pull request whose **entire body is exactly
one line** in one of these forms:

```text
/searchkernel-verify ci <exact-40-character-sha>
/searchkernel-verify test <scope> <exact-40-character-sha>
/searchkernel-verify verify <lane> <exact-40-character-sha>
```

Rules:

- use the exact pushed SHA being reviewed, never a branch name or abbreviated SHA;
- use only a registered `test` scope or `verify` lane from
  `build/targets.json`;
- do not add explanatory prose, Markdown, a second line, or another command to
  the same comment;
- the comment author must be accepted by the repository's bounded authorization
  policy (`OWNER`, `MEMBER`, or `COLLABORATOR`);
- after posting, locate the resulting **On-demand verification** workflow run
  and inspect its structured evidence and failed-step logs when applicable.

The PR comment is only a request envelope. It cannot supply arbitrary shell
text, workflow YAML, runner configuration, or secrets. The trusted default-
branch control plane validates the request and executes the exact requested
revision as untrusted subject code.

#### Repository-dispatch ingress

If the caller can issue the repository's bounded `repository_dispatch`, it may
instead dispatch the **On-demand verification** workflow with one operation:

- `ci` with no target;
- `test` with one registered test scope;
- `verify` with one registered verifier lane.

The dispatch path and PR-comment path normalize into the same exact-SHA verifier
and have the same target-registry, repository-membership, checkout, execution,
and evidence requirements.

For either ingress, confirm the workflow evidence identifies the
requested/observed revision, operation, canonical target/command, result, run
ID, and run URL. For a PR comment request, also confirm the trusted reporter
posts the PASS/FAIL result back to that PR with a clickable Actions run link.
Inspect failed-step logs when the result is not successful. Never treat a
successfully posted request comment or accepted dispatch as proof that the test
itself passed.

### Evidence freshness

Verification evidence is valid only for the exact commit SHA it names. A new
commit or push creates a new head and invalidates older runs as evidence for the
current change, even when the diff seems unrelated.

After every fix or new commit, rerun every check required for acceptance against
the new exact head. Never claim that the current PR/head was verified by a run
for an older commit or by a mutable branch reference.

## Follow-up work

Review follow-up uses this durable state transition:

```text
issue: status:needs-followup
    ↓
issue: status:in-progress
    ↓
PR: needs-cto-review
```

When review requests a repair:

- keep/convert the pull request to Draft when it is not ready;
- remove `needs-cto-review` from the pull request if present;
- mark the issue `status:needs-followup`;
- leave actionable feedback describing the defect, expected behavior, repair
  direction, and required verification.

A follow-up worker:

1. identifies the existing open pull request;
2. changes the issue from `status:needs-followup` to
   `status:in-progress`;
3. comments exactly: `Claimed follow-up; working on PR #X.`;
4. updates the existing pull request branch rather than creating a replacement;
5. performs only the requested repair or missing verification.

Successful handoff requires a PR comment with what changed, exact verification
commands/results, and the tested head SHA. Then mark the PR Ready for review,
remove `status:in-progress` from the issue, and add `needs-cto-review` to the
pull request.

If required verification still fails, keep the issue in progress, record the
exact blocker, and do not mark the PR ready or add `needs-cto-review`.

Follow-up workers do not approve, merge, close, or declare their own work
VERIFIED/CLOSED.

## Implementation discipline

Make the smallest good change that satisfies the task and architecture.

Do not:

- invent public APIs when a frozen API exists;
- change acceptance artifacts merely to make implementation pass;
- hard-code fixture IDs, queries, scores, documents, or expected results;
- special-case verifier input;
- introduce a fake production backend;
- patch upstream Vespa;
- hand-edit generated SDK/protocol output;
- add abstractions without demonstrated need;
- add production dependencies without a concrete reason;
- perform unrelated renames, formatting sweeps, or dependency churn.

Prefer existing repository patterns over new conventions.

When a non-obvious invariant is discovered, leave a durable breadcrumb in the
narrowest useful place: local comment, doc comment, or existing architecture
document. Document the reason or constraint, not a narration of the code.

Good breadcrumbs include ownership/lifetime assumptions, Vespa-required
behavior, ordering/concurrency/persistence constraints, tempting but incorrect
simplifications, and non-obvious verification requirements.

Do not leave conversation notes, temporary implementation plans, speculation,
or duplicated explanations.

## Commit discipline

Completed implementation work is committed and pushed to its task branch unless
the user explicitly asks otherwise.

Before committing:

1. run the required focused verification;
2. inspect `git status --short`;
3. inspect the complete task diff;
4. stage only task-owned changes;
5. inspect `git diff --cached`;
6. commit.

Commit messages describe the change, not agent activity. Follow the repository's
recent convention; if none is clear, use a concise Conventional Commit-style
subject such as:

```text
fix(grouping): preserve nested aggregation results
refactor(ir): split grouping resolution
docs(agents): slim root agent guidance
```

Do not amend, rebase, squash, or rewrite commits belonging to another task.

Push the task branch after verification. Do not push implementation commits
directly to `main` unless the user explicitly instructed direct-main work.

## Pull request handoff

Normal integration is:

```text
issue
  ↓
claim + task branch/worktree
  ↓
regression reproduction when fixing a bug
  ↓
implementation + verification
  ↓
commit + push
  ↓
pull request into main
  ↓
review / follow-up
  ↓
merge
  ↓
issue closure
```

Every implementation pull request should include:

- the linked issue;
- a concise summary of the actual change;
- relevant architecture/contract constraints;
- regression reproduction evidence for reported bugs;
- exact verification commands and results;
- required acceptance/verifier results;
- remaining limitations or follow-up work.

Use `Closes #<issue>` only when merge fully resolves the issue.

The pull request is the durable implementation report. Do not create a separate
agent-report file.

## Review expectations

Implementation completion is not closure. For correctness-sensitive changes,
review should try to falsify the change rather than only confirm the happy path.
Depending on blast radius, consider:

- races/interleavings;
- malformed input;
- absent vs empty state;
- persistence/reopen;
- alternate ingress paths;
- generated artifact drift;
- error-domain mismatch;
- architectural bypasses or silent fallback;
- whether the tested code is actually the merged/current code.

Review depth should scale with blast radius. Mechanical documentation-only
changes do not need the same adversarial depth as native lifecycle or semantic
changes.

Actionable rejection/review feedback should state:

1. what is wrong;
2. evidence;
3. expected behavior;
4. a concrete repair direction;
5. how to verify the repair.

If the repair is independently reviewable and materially larger than the
original atomic issue, create a follow-up issue rather than silently expanding
the task.

## Completion states

A code task has two distinct states.

### Ready for review

Ready means:

- implementation matches the issue scope;
- required regression reproduction exists for reported bugs;
- architecture/contracts are preserved;
- focused verification passes;
- required acceptance/verifier checks pass;
- broader verification ran when warranted;
- the final diff was reviewed;
- task-owned work is committed and pushed;
- a PR into `main` contains verification evidence.

An implementation agent may say a PR is ready for review. It does not approve
its own work or declare it VERIFIED/CLOSED.

### Closed

Closed means the resolving pull request was reviewed, accepted, merged, and the
issue is actually resolved.

Passing tests alone is not closure if the implementation violates a required
native path, API contract, semantic boundary, repository rule, or review
requirement.

When handing work back, report concisely:

- issue number;
- pull request URL/number;
- what changed;
- important verification commands/results;
- commit hash/subject;
- whether the PR is ready for review or merged;
- any remaining blocker or unsupported capability;
- relevant pre-existing state.

Do not claim the repository is clean unless it actually is.
