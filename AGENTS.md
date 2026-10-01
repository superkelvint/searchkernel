# AGENTS.md

## Project

Search Kernel is a thin Rust interface to Vespa's local C++ search engine.

Search Kernel owns the product API, portable semantics, Rust/native boundary,
integration, packaging, and fidelity tests. Vespa owns the search engine:
persistence, indexing, attributes, matching, query execution, ranking, tensors,
vectors, concurrency, and low-level resource management unless a task explicitly
says otherwise.

When choosing between faithfully exposing Vespa and inventing a more general
Search Kernel subsystem, prefer Vespa.

## Read first

Before a non-trivial change, read only the authorities relevant to the task:

- `docs/reading-the-codebase.md` — repository navigation and request flow;
- `docs/architecture.md` — current architecture and dependency boundaries;
- `BUILDING.md` — canonical build, test, verification, toolchain, and recovery
  instructions;
- `docs/change-recipes.md` — propagation paths for API/IR/native/client changes;
- the relevant contract/specification under `reference/` or `docs/`.

For issue claiming, worktrees, follow-up lifecycle, commit/PR process, and review
handoff, read `docs/agent-workflow.md`.

Follow a more-specific `AGENTS.md` when one exists in the subtree you are
editing.

Do not duplicate large architecture, build, or workflow specifications here.
Repository documentation and executable contracts are the source of truth.

## Repository map

```text
api/                 canonical FlatBuffers portable API
core/crates/         Rust implementation
core/native/         Search Kernel C/C++ Vespa adapter
clients/             non-Rust SDKs
searchkerneld/       daemon / transport host
searchkernel-web/    administration dashboard
upstream/vespa/      pinned Vespa source; read-only
reference/           frozen contracts, corpora, oracles, verifiers
tools/               build/generation/verification tooling
docs/                architecture and development documentation
```

## Architectural invariants

These are non-negotiable unless the task explicitly changes the architecture:

- FlatBuffers is the sole canonical portable domain contract.
- `searchkernel-ir` owns portable semantics through the shared resolver /
  `*_to_ir()` path.
- There must be one implementation of portable semantics.
- Rust uses the engine directly in-process by default.
- First-party non-Rust clients use canonical FlatBuffers through thin transports.
  Binary FlatBuffers over generated gRPC is the default native/runtime path;
  explicitly selected HTTP clients use standard FlatBuffers JSON only through
  the official BFBS-backed bridge into the same canonical dispatcher.
- Protobuf is transport framing only; it does not duplicate the domain model.
  HTTP routes and JSON DTOs must not define a parallel domain model.
- `searchkernel-ir-adapter` is the integration boundary between portable IR and
  the Vespa-facing engine.
- Vespa types stay behind the native adapter.
- The retired portable client FFI/native-transport architecture must not return.
- Do not independently implement Vespa engine behavior in Rust merely to make a
  test pass.

Prefer the smallest correct design that preserves these boundaries.

## Behavioral authority

For wrapper behavior, authority is:

1. frozen API/milestone contract;
2. pinned Vespa source and tests;
3. supplied direct oracle/golden behavior;
4. Search Kernel implementation.

The implementation does not define expected behavior. If contract and Vespa
behavior appear incompatible, surface the conflict rather than silently
redefining either one.

## Starting a task

For a concrete task assigned directly by the user, do not search the wider issue
queue first. If the user names an issue or pull request, fetch that item directly.

For a non-trivial implementation task with no existing issue, create one for the
exact requested task before editing. Implementation normally happens on a
dedicated task branch/worktree and is delivered through a pull request into
`main`. Detailed queue labels, claiming, worktree, and handoff rules are in
`docs/agent-workflow.md`.

Before editing:

1. read the task issue/PR when one exists;
2. read the relevant implementation, tests, contract, and existing pattern;
3. identify the layer that owns the behavior;
4. for Vespa behavior, inspect pinned source/tests under `upstream/vespa/`;
5. identify the smallest focused verification that will prove the change;
6. avoid unrelated cleanup.

### Reported bugs: regression test first

For a reported bug, defect, regression, or incorrect behavior, the first code
change must be the smallest executable regression test that reproduces the real
affected path.

Required order:

1. add the regression test;
2. run it against the current implementation and confirm the expected failure;
3. only then change production code;
4. rerun the regression and confirm it passes;
5. run the broader verification required by the affected area.

A source-text assertion or a test that already passed before the fix is not a
valid regression reproduction. If the issue cannot be reproduced, continue
investigating or report the failed reproduction evidence instead of guessing at
a fix.

## Build and verification

`BUILDING.md` is authoritative for setup, canonical commands, toolchains,
native SDK/runtime provisioning, and environment recovery. The exact-SHA remote
verification protocol for local-capable and remote-only agents is documented in
`docs/agent-workflow.md`.

A remote-only agent with authorized GitHub pull-request comment access can run
canonical checks without a local Search Kernel environment or
`repository_dispatch` access. Post exactly one top-level PR comment whose
entire body is one of:

```text
/searchkernel-verify ci <exact-40-character-sha>
/searchkernel-verify test <scope> <exact-40-character-sha>
/searchkernel-verify verify <lane> <exact-40-character-sha>
```

Use a pushed exact SHA and a registered target from `build/targets.json`. Do
not add prose to the command comment. Treat the resulting exact-SHA workflow
evidence as the test result, and rerun required checks after every new head
commit. See `docs/agent-workflow.md` for authorization, evidence, and failure
handling.

Use the repository's `./dev` interface and the target vocabulary in
`build/targets.json`; do not create a second command matrix in code, CI, or
documentation.

In an unfamiliar environment:

1. read `BUILDING.md`;
2. start environment recovery with `./dev doctor`;
3. fix/report prerequisite failures before changing product code.

Do not improvise environment repairs such as `cargo clean`, deleting target
directories, copying SDKs between environments, or rebuilding immutable caches
unless the documented recovery path instructs it.

During implementation, run the narrowest useful test/verifier repeatedly, then
expand verification according to blast radius. Also:

- run the exact acceptance/verifier command when the task defines one;
- verify generated artifacts when changing their source;
- use a real Vespa-backed path when the contract requires engine behavior;
- verify persistence/reopen when persistent behavior changes;
- verify both ingress paths when shared portable semantics change;
- require zero compiler and Clippy warnings.

For native-affecting changes, cheap PR CI alone is not evidence that real Vespa
behavior was exercised. Run or dispatch the applicable native/real-engine lane
using local `./dev test` / `./dev verify` or the exact-SHA remote protocol in
`docs/agent-workflow.md`.

If a required check cannot run, record the exact command, failure, and reason.
Never claim a check passed when it did not run.

## Search scope

Repository-wide discovery should respect `.rgignore` by default.

Do not include generated FlatBuffers/client output, BFBS files, native SDK
artifacts, build output, or caches in normal code discovery. Search excluded
paths explicitly only when debugging generation, protocol encoding, generated
bindings, packaging, or native SDK/ABI output.

## Implementation discipline

Make the smallest good change that satisfies the task and architecture.

Do not:

- invent public APIs when a frozen API exists;
- weaken/change acceptance artifacts merely to make code pass;
- hard-code fixture-specific production behavior;
- special-case verifier input;
- introduce fake production backends;
- patch `upstream/vespa/`;
- hand-edit generated SDK/protocol output;
- add abstractions or production dependencies without demonstrated need;
- perform unrelated renames, formatting sweeps, or dependency churn.

Prefer existing patterns and obvious ownership boundaries.

When investigation reveals a non-obvious invariant that future maintainers would
otherwise have to rediscover, leave a durable breadcrumb in the narrowest useful
place. Document the reason, constraint, lifetime/ordering rule, Vespa behavior,
or required verification—not the agent's process or a narration of obvious
code.

## Vespa and native boundary

`upstream/vespa/` is read-only. Determine the pinned revision through the
repository's authoritative metadata/tooling; do not silently update it.

Search Kernel-specific C/C++ adaptation belongs under:

```text
core/native/vespa/
```

Do not spread Vespa headers or types into ordinary Rust code. The public native
boundary uses Search Kernel-owned primitives and opaque handles.

At every native boundary, ownership must be explicit:

```text
who allocates
who owns
how long borrows remain valid
who frees
whether concurrent access is allowed
```

Keep raw `unsafe` at the dedicated native/bindings boundary. Catch C++
exceptions inside the adapter; Rust panics must not unwind into C++. Prefer
simple safe copies over fragile zero-copy lifetime schemes unless measurement
shows the copy matters.

Native SDK acquisition, provenance, cache behavior, and recovery belong in
`BUILDING.md`, not duplicated here.

## Reference and generated artifacts

Contracts, scenarios, expected outputs, oracles, comparators, and verifier
artifacts under `reference/` are frozen unless the task explicitly asks to
author or repair them.

Do not weaken, regenerate, or rewrite acceptance truth merely to make the
implementation pass. Scripted/mock backends can prove focused wrapper behavior;
they do not prove a real-engine contract that requires Vespa.

Generated files are outputs, not semantic authorities. Change their source and
regenerate through the repository tooling.

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
- deterministic conversion/ordering where contracts require it.

Avoid unnecessary `Arc`, `Mutex`, cloning, interior mutability, polymorphic
hierarchies, broad lint suppressions, or clever abstractions. Do not use
`unwrap()`/`expect()` for malformed input or normal operational failure.

Do not optimize away simple safe code without evidence.

## Integration and handoff

Implementation integrates through a pull request into `main` unless the user
explicitly directs otherwise.

The pull request is the durable implementation report. It should contain:

- the linked issue;
- a concise description of the actual change;
- important architecture/contract constraints;
- regression reproduction evidence for reported bugs;
- exact verification commands/results;
- remaining limitations or follow-up work.

An implementation agent may say its pull request is ready for review. It does
not approve its own work or declare it VERIFIED/CLOSED. Final acceptance and
merge are review decisions.

Before handing off a substantive change, review the final diff for correctness,
contract compliance, architectural boundaries, regressions, error handling,
unintended scope, and missing tests.

Detailed issue state transitions, follow-up handling, commit discipline, PR
review, and completion-state rules are in `docs/agent-workflow.md`.
