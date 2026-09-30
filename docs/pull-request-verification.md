# Pull request verification

Search Kernel keeps mandatory pull-request CI intentionally small and runs
specialist verification explicitly against exact revisions.

## Mandatory PR gate

The registered PR workflow is `.github/workflows/rust-ci.yml`. It checks out the
GitHub event revision and delegates the entire mandatory gate to:

```sh
./dev ci
```

`./dev ci` is the semantic owner of the PR gate. GitHub Actions owns scheduling
and checkout only; it must not duplicate compiler flags, toolchain setup,
verification selection, or product semantics.

A head-changing operation such as a rebase, force-push, or conflict resolution
creates a new revision and therefore requires a new CI run for that revision.

## Focused and specialist verification

Checks beyond the mandatory PR gate are selected by the implementer/reviewer from
the canonical registry in `build/targets.json`:

```sh
./dev test --list
./dev test <scope>

./dev verify --list
./dev verify <lane>
```

The repository does not automatically expand ordinary PR CI into a heavy
path-selected verification matrix. Reviewers choose the checks that can falsify
the changed behavior, with stronger native, persistence, sanitizer, packaging,
or cross-language evidence when the affected boundary requires it.

## Exact-SHA remote verification

When a check cannot be run locally, dispatch
`.github/workflows/on-demand-verification.yml` with an exact commit SHA and one
bounded operation:

```text
ci
test <scope>
verify <lane>
```

The trusted workflow validates `test` and `verify` targets directly from the
requested revision's `build/targets.json`, checks out that revision separately as
the subject under test, proves the checked-out SHA, and executes the matching
canonical `./dev` command.

Local-capable callers can use:

```sh
./dev test --remote <scope>
./dev verify --remote <lane>
```

Evidence for one SHA is not evidence for another SHA.

## Review rule

CI is evidence, not the review. For substantive changes, reviewers still need to
check architecture, compatibility, malformed/boundary inputs, false-green
mechanisms, cross-language parity, and whether the chosen focused verification
actually exercises the production path.

The canonical build and verification architecture is documented in
`implementation-plan/build-system.md` and `docs/ci.md`.
