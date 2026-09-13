# AGENTS.md

## Project scope

Search Kernel is a **thin Rust wrapper around the local C++ search
engine used by Vespa**.

The intended layering is:

```text
public Rust API
    -> safe Rust wrapper
    -> raw FFI/sys crate
    -> private Search Kernel native ABI
    -> Search Kernel C++ adapter
    -> Vespa C++
```

Search Kernel owns the wrapper: Rust API, ownership/lifetimes, conversion,
errors, startup/shutdown, build/link/package integration, language bindings,
and fidelity tests.

Vespa owns the search engine unless a task explicitly says otherwise. This
includes persistence, transaction logging/recovery, indexes, fusion,
attributes, matching, query execution, ranking, BM25, tensors, vectors,
concurrency, and low-level resource management.

When choosing between:

```text
represent Vespa faithfully
```

and:

```text
invent a more general Search Kernel subsystem
```

prefer the first until real usage proves the second is necessary.

## Repository boundaries

```text
searchkernel2/
├── crates/                 # Rust implementation
├── native/
│   ├── vespa/              # our C/C++ adapter
│   └── sdk/                # generated/prebuilt native SDK
├── upstream/
│   └── vespa/              # pinned upstream Vespa checkout; read-only
├── reference/
│   └── vespa/              # contracts, golden data, oracles, verifiers
├── tools/
├── docs/
└── AGENTS.md
```

### `upstream/vespa/` is read-only

The Vespa checkout is intentionally inside the repository so sandboxed agents
can inspect it.

Pinned revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

Use `upstream/vespa/` as the source of truth for Vespa behavior. Inspect the
actual source and tests when wrapping a capability; do not guess from names or
high-level documentation.

Do not modify tracked files under `upstream/vespa/`.

When relevant, verify:

```bash
git -C upstream/vespa rev-parse HEAD
git -C upstream/vespa diff --quiet
git -C upstream/vespa diff --cached --quiet
```

The revision must match the pin above and tracked source must be clean. Untracked
build artifacts may exist.

If the checkout is wrong or tracked source is dirty, report it. Do not fix a
milestone by patching Vespa itself.

### `native/vespa/` is our adaptation layer

All Search Kernel-specific C/C++ integration belongs here.

If the Vespa API is awkward, adapt it in `native/vespa/`; do not spread Vespa
headers/types into Rust or modify upstream source.

### `native/sdk/` is generated

Normal Rust work consumes the prebuilt SDK and should not require a Vespa build
or Docker session.

Do not hand-edit SDK contents. Rebuild them only when the task explicitly
requires a native adapter/SDK change, using repository tooling.

## Source of truth and milestone discipline

For wrapper behavior, authority is:

```text
1. frozen milestone/API contract
2. pinned Vespa source and tests
3. direct C++ oracle/golden behavior, where supplied
4. Search Kernel implementation
```

The implementation does not define expected behavior.

Milestones are verifier-first. The repository may contain:

```text
reference/vespa/API_CONTRACT.md
reference/vespa/api-contract/
reference/vespa/mN/CONTRACT.md
reference/vespa/mN/scenarios/
reference/vespa/mN/expected/
reference/vespa/mN/oracle/
reference/vespa/mN/SOURCE_TRACE.md
reference/vespa/mN/verify-mN.sh
reference/vespa/verify-through.sh
```

Unless the task explicitly asks to author or revise the harness, these are
**checked-in acceptance artifacts**. The verifier-authoring commit is the
specification freeze boundary.

Do not modify contracts, API fixtures, scenarios, expected outputs, oracles,
comparators, or verifier scripts merely to make implementation pass. Git diff
and review are the enforcement mechanism; implementation agents may change
these files only when explicitly asked to author or repair a verifier.

For a reviewer, the useful checks are:

```bash
git diff -- reference/vespa reference/clients reference/integration
git diff <verifier-freeze-commit>..HEAD -- reference/
```

Do not weaken earlier milestone tests. A later milestone must preserve all
completed earlier milestones.

If a task provides an acceptance command, run that exact command before calling
the task complete.

## The Rust API is defined up front

Agents implementing milestones do **not** get to invent the public API.

The API contract and compile fixtures under `reference/vespa/` are normative.
If they require:

```rust
let engine = Engine::open(path, config)?;
```

then this is not equivalent:

```rust
let engine = VespaClient::new(...)?;
```

Do not rename public types, alter ownership, change return types, add required
arguments, or replace the specified interface unless the task explicitly
authorizes an API change.

If a frozen API cannot faithfully expose the Vespa capability, surface the
conflict instead of silently changing the API or verifier.

## No shortcuts

Passing a fixture is not sufficient if the implementation bypasses the intended
Vespa path.

Unless explicitly authorized:

- do not replace Vespa with a mock/in-memory production backend;
- do not implement matching in Rust to avoid Vespa matching;
- do not compute BM25/ranking independently in Rust when wrapping Vespa ranking;
- do not hard-code fixture IDs, documents, queries, scores, or results;
- do not special-case verifier inputs;
- do not return precomputed golden output;
- do not bypass the native adapter;
- do not patch upstream Vespa to simplify the wrapper;
- do not weaken lifetime/error handling to make tests pass;
- do not add a second production backend just to simplify tests.

Mocks/fakes may be used for focused Rust unit tests, but they do not prove a
Vespa integration milestone. Acceptance tests must exercise the real native
engine when required by the contract.

A direct C++ oracle is a behavioral reference, not another production backend.

## Native boundary rules

### Public native ABI aggregate rule

The public native ABI must use opaque handles and fixed-width primitive
arguments. Text or other repeated data must use primitive arrays (preferably a
single structure-of-arrays transfer), never a public aggregate struct. A
future aggregate may cross the boundary only after an executable benchmark
demonstrates that the primitive representation is materially inadequate; a
source-text check or intuition is not evidence. The reason is architectural:
an aggregate creates a second C/Rust layout contract and, when it contains
borrowed pointers, a second lifetime contract. New ABI additions must follow
this rule as well.

### No Vespa types in the public Rust API

The native ABI uses Search Kernel-owned values and opaque handles. Convert to
Vespa-specific types inside the C++ adapter.

Do not expose Vespa headers, namespaces, pointers, ownership conventions, or
internal classes through the Rust-facing API.

### Keep `unsafe` at the boundary

Raw FFI belongs in the dedicated sys/raw crate.

The safe wrapper must establish clear invariants for:

```text
handle ownership and exactly-once destruction
pointer/null validity
borrowed buffer/string lifetimes
array lengths/capacities
thread-safety assumptions
error payload ownership
```

Do not spread `unsafe` into ordinary API code for convenience.

### Exceptions and panics do not cross the ABI

Catch C++ exceptions inside the adapter and translate them to the native error
representation.

Rust panics must not unwind into C++.

Operational failures become Rust `Result` errors. Do not abort or `unwrap()` on
malformed input, initialization failure, I/O failure, or ordinary Vespa errors.

### Ownership is explicit

For every value crossing the boundary, know:

```text
who allocates it
who owns it
how long borrows remain valid
who frees it
whether calls may be concurrent
```

Prefer safe, obvious copies at the boundary until measurement shows a large
copy matters. Do not introduce fragile zero-copy lifetimes prematurely.

## Wrap before abstracting

Prefer:

```text
Vespa operation
    -> thin native adapter
    -> idiomatic Rust value
```

over inventing a generic query framework, schema framework, backend trait,
plugin system, ranking VM, storage abstraction, or dependency-injection layer.

Start with concrete Rust types that faithfully represent the Vespa capability.
Extract abstractions only after multiple real uses justify them.

Convenience APIs may come later. Fidelity comes first.

## Use engineering judgment

Implement the **simplest correct design appropriate for a native Vespa wrapper**,
not the cheapest implementation that passes the current fixture.

"Simple first" does not mean "throwaway first."

Do not knowingly choose a shape that must be demolished by the immediately
following milestone. But do not speculate about distant requirements or build
frameworks for them.

When two implementations are similarly straightforward, prefer the one that:

- follows the frozen API;
- stays close to real Vespa semantics;
- keeps the native boundary narrow;
- isolates upstream churn inside `native/vespa/`;
- leaves the next known milestone additive.

The goal is the smallest **good** implementation.

## Commit discipline

Commit in bite-sized, reviewable increments, not one large end-of-task dump.
Examples:

```text
add opaque native engine handle and destruction
wrap open/close in safe Rust ownership
add M2 document conversion
add native search-result conversion
```

Each commit must:

- stay within authorized task/file scope;
- remain buildable whenever reasonably possible;
- include focused tests for new behavior;
- explain the change in the commit message;
- avoid unrelated cleanup, renaming, formatting, or dependency churn;
- never edit checked-in acceptance artifacts unless the task is verifier-authoring work;
- never modify upstream Vespa source.

## Regression test quality

When a reported defect is fixed, first add a regression test that demonstrates
the defect against the existing implementation and confirm that it fails. The
test must exercise observable behavior or the actual implementation path.
Never use source-text searches, `include_str!` assertions, naming conventions,
or similar textual proxies as a substitute for an executable regression test.
Such tests are not evidence that the defect exists or that the fix works.

## Rust style

Write idiomatic, production-quality Rust for a native systems wrapper.

### Ownership

Do not defeat the borrow checker with cloning, `Arc`, complicated lifetimes, or
interior mutability.

Prefer:

```text
single clear owner
+ borrowed access where natural
+ explicit ownership transfer
```

`Arc<Mutex<Runtime>>` is not an ownership architecture.

### Concrete types first

Do not turn every noun into a trait. Avoid factories, registries, service
objects, backend hierarchies, and `Box<dyn Trait>` without a real polymorphic
need.

In particular, do not invent an `EngineBackend` abstraction merely to put a
fake implementation next to Vespa. Vespa is the production backend.

### Allocation

Avoid gratuitous cloning and intermediate allocation, especially around large
documents, result sets, feature vectors, and native buffers.

Do not prematurely optimize away clear and safe boundary copies without
measurement.

### Errors

Use `Result` for operational failure and `Option` for absence.

`unwrap()`/`expect()` are for true internal invariants, not malformed input,
native errors, missing files, invalid config, or runtime failure.

Do not log an error and continue as though the operation succeeded.

### Determinism

Do not introduce wrapper-level nondeterminism where Vespa provides deterministic
observable behavior.

Do not rely on `HashMap` iteration order, thread scheduling, pointer values, or
incidental local/native ordering during result conversion.

When Vespa intentionally leaves ordering unspecified, do not invent a stronger
Rust guarantee.

## Definition of done

At minimum:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

There must be zero compiler and Clippy warnings.

Do not weaken lint settings or broadly suppress warnings to pass.

For native C/C++, compile cleanly under the repository's configured warning
policy.

When the task supplies a milestone acceptance command, run it as well.
Compilation/unit tests alone do not replace milestone verification.

Passing tests is not completion if the implementation violates the required
native path, public API contract, or repository boundaries.

## Normal development workflow

The ordinary loop should not require rebuilding Vespa:

```bash
cargo test --workspace
```

Agents may inspect `upstream/vespa/` at any time.

Do not enter Docker or rebuild the SDK unless the task actually requires a
change to `native/vespa/`, SDK construction, or a native capability absent from
the current SDK.

When a native rebuild is required, use repository tooling against the pinned
`upstream/vespa/` checkout. Do not hand-edit generated outputs.

## Final milestone review

Before declaring a milestone complete, verify:

```text
API
- matches frozen API contract
- no unapproved public API invention

Vespa fidelity
- real pinned Vespa path is used
- no Rust reimplementation or fixture-specific shortcut

Boundary
- Vespa types stay behind native/vespa
- raw unsafe stays in the sys/raw layer
- ownership and errors are explicit
- exceptions/panics cannot cross the ABI

Repository
- tracked upstream/vespa source is unchanged
- checked-in reference/verifier artifacts are unchanged
- SDK outputs were not hand-edited

Quality
- fmt passes
- clippy passes with warnings denied
- workspace tests pass
- milestone verifier passes
- earlier milestones still pass
```
