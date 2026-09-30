# Acceptance and reference guidance

The material under `reference/` defines behavioral acceptance for the wrapper
and portable clients. The implementation does not define expected behavior.

## Authority and oracle use

For wrapper behavior, use this order of authority:

```text
1. frozen milestone/API contract
2. pinned Vespa source and tests
3. direct C++ oracle/golden behavior, where supplied
4. Search Kernel implementation
```

A direct C++ oracle is a behavioral reference, not another production backend.
Acceptance tests must exercise the real native engine when the contract
requires it. Do not replace Vespa with a mock/in-memory production backend, do
not implement matching in Rust, compute BM25/ranking independently of Vespa,
or bypass the native adapter.

## Frozen verifier material

Verifier-first material may include:

```text
reference/vespa/API_CONTRACT.md
Historical API compile-contract assertions now live in the owning core crate examples.
reference/vespa/mN/CONTRACT.md
reference/vespa/mN/scenarios/
reference/vespa/mN/expected/
reference/vespa/mN/oracle/
reference/vespa/mN/SOURCE_TRACE.md
reference/vespa/mN/verify-mN.sh
reference/vespa/verify-through.sh
```

Contracts, API fixtures, scenarios, expected outputs, oracles, comparators, and
verifier scripts are checked-in acceptance artifacts unless a task explicitly
asks to author or repair a verifier. The verifier-authoring commit is the
specification freeze boundary. Do not modify or weaken these artifacts merely
to make an implementation pass.

Git diff and review enforce this boundary. Implementation work may change
these files only when explicitly authoring or repairing a verifier.

Do not:

- hard-code fixture IDs, documents, queries, scores, or results;
- special-case verifier inputs or return precomputed golden output;
- weaken lifetime/error handling to make tests pass;
- add a second production backend just to simplify tests.

Mocks and fakes are appropriate for focused unit tests, but they do not prove
an integration milestone. Preserve all completed earlier milestones when
adding a later one.

Useful review checks are:

```bash
git diff -- reference/
git diff <verifier-freeze-commit>..HEAD -- reference/
```

If a task provides an acceptance command, run that exact command before calling
the task complete. Compilation and unit tests do not replace milestone
verification.
