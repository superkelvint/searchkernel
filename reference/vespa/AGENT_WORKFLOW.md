# Agent workflow

## Before implementation

The maintainer prepares and commits:

```text
CONTRACT.md
API_CONTRACT.md
scenarios/
expected/
oracle/
verify scripts
```

The reviewed verifier and expected outputs are committed as the milestone's
specification freeze boundary. The implementation agent is told that the
checked-in `reference/vespa/**` verifier files are frozen during implementation.

## Implementer prompt template

```text
Implement M<N> of Search Kernel.

Acceptance command:

    ./reference/vespa/verify-through.sh m<N>

M0..M<N-1> are complete and must not regress.

The checked-in verifier specification is frozen. Do not modify contracts, API
fixtures, scenarios, expected outputs, oracle sources, comparators, or
verifier scripts unless explicitly doing verifier-authoring work.

Implement only the behavior required by reference/vespa/m<N>/CONTRACT.md and
the public API frozen in reference/vespa/API_CONTRACT.md.

Do not:
- replace Vespa behavior with a Rust implementation;
- add fixture-specific behavior;
- hard-code golden IDs or values;
- bypass the native adapter;
- weaken error handling;
- change the API to make implementation easier;
- regenerate expected data.

Study the pinned Vespa source where necessary.
Keep the implementation minimal.
```

## Reviewer mandate

A separate reviewer should not fix code initially. It should look specifically
for shortcuts:

```text
- search or mutation behavior reimplemented in Rust
- fixture-specific branches or constants
- fake/memory backend used by acceptance path
- native library linked but not actually used
- unsafe lifetime or ownership shortcuts
- swallowed native errors
- Vespa types leaking into the public API
- API contract bypassed with alternate entry points
- verifier/reference files modified
- broad abstractions added beyond the milestone
```

The reviewer returns findings only.

## Fixer

The fixer receives the verifier failure plus reviewer findings and changes only
implementation files.

## Verifier

The final agent runs:

```sh
./reference/vespa/verify-through.sh m<N>
```

and reports the exact command and exit status.

## CI

When CI knows the verifier-freeze commit, it may independently enforce:

```sh
git diff --exit-code <verifier-freeze-commit>..HEAD -- reference/
```

Local development does not require a hook or cleanliness lock; ordinary Git
review is the protection boundary.

Before implementation, review and commit the verifier specification. For a
reviewer, inspect reference changes with:

```sh
git diff -- reference/vespa reference/clients reference/integration
git diff <verifier-freeze-commit>..HEAD -- reference/
```
