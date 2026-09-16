# Frontend milestone harness

This is the immutable acceptance boundary for the future F-series dashboard.
It does not contain dashboard code or Search Kernel semantics.

## Workflow

```text
frontend-milestone begin F0
frontend-milestone candidate F0
frontend-verify F0
frontend-accept F0
```

`frontend-accept` is the only command that can set a milestone to `passed`.
Dependencies are loaded from the milestone manifests. All initial F-series
verifiers intentionally return `NOT_IMPLEMENTED`, so no milestone is currently
accept-able.

## Integrity boundary

Each manifest records a verifier-freeze Git commit in `freeze_commit`. The
maintainer records that commit after verifier authoring and before implementation
begins. The integrity command rejects protected changes, deletion, and untracked
protected files. `state.json` is operational state and is not verifier material.

Protected material includes manifests, verifier trees, schemas, and reviewer
contracts. Runtime review and verification results belong under `target/`.

## Architecture checker

The checker is deliberately mechanical. It scans existing TypeScript/JavaScript
dashboard source, excluding `src/admin-client/**` from portable-network checks,
and scans existing Cargo manifests. It can miss dynamic imports, generated code,
aliased packages, or semantics hidden behind macros. Passing it never replaces
the independent reviewers or milestone verifier.

The approved admin client location is `searchkernel-web/src/admin-client/**`.
It may call diagnostic/admin endpoints; portable operations must use the shared
generated TypeScript client.

## Results

Reviewer results are read from
`$FRONTEND_REVIEW_DIR/<milestone>/<reviewer>.json`, defaulting to
`target/frontend-harness/reviews/`. Missing or malformed required reviews block
acceptance. Warnings are reported but do not block.

The clean mode verifies the committed `HEAD` in a detached temporary worktree.
Acceptance additionally requires the caller's worktree to be clean, including
no untracked files.
