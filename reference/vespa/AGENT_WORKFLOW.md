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

The implementation agent is told that `reference/vespa/**` is immutable.

## Implementer prompt template

```text
Implement M<N> of Search Kernel.

Acceptance command:

    ./reference/vespa/verify-through.sh m<N>

M0..M<N-1> are complete and must not regress.

The entire reference/vespa directory is immutable. Do not modify contracts,
API fixtures, scenarios, expected outputs, oracle sources, comparators, or
verifier scripts.

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

CI should independently enforce:

```sh
test -z "$(git status --porcelain -- reference/vespa)"
./reference/vespa/verify-through.sh m<N>
```

Do not rely only on instructions in the Codex prompt to keep the harness clean.
