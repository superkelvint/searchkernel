# Verification DAG

`verification/dag.toml` is SearchKernel's semantic verification authority. It
describes what verification exists and how its closure is composed; GitHub
Actions, moonrepo, pre-commit, and other execution systems are projections of
this model rather than independent sources of verification meaning.

Task kinds:

- `command` — executable verification with a canonical repository command.
- `aggregate` — a commandless closure over other tasks.
- `unverified` — an explicitly missing verification capability. It must carry
  a reason; it is never silently treated as a passing command.

Edges have two meanings:

- `depends_on` — an executor prerequisite that should be scheduled separately.
- `subsumes` — the task's current command already executes/proves that task
  closure. This records cumulative historical verifiers without telling a
  scheduler to run the same work twice.

Historical milestone names are aggregate aliases such as `milestone.m14`.
Semantic executable identities describe behavior, such as
`vespa.nearest-neighbor.hnsw` or `vespa.persistence.reopen`.

The trusted exact-head selector still emits several pre-DAG IDs. Those are
recorded under `[[selector_alias]]` and mechanically checked until the selector
is migrated. New DAG work must not introduce another historical leaf identity.

Validate the graph with:

```bash
./dev verify verification-dag
```

The validator fails closed for malformed schema, unknown fields, duplicate IDs,
missing edges or commands, cycles, unreachable required tasks, stale selector
aliases, semantic verifier drift, and newly introduced Rust crates or web feature
families without explicit subsystem ownership.

An `unverified` node in the full closure is intentional evidence that the
architecture has verification ownership but no executable proof yet. Selection
of a subsystem requiring such a node must eventually block merge when the
trusted selector is migrated to this DAG; it must not be interpreted as a green
test.
