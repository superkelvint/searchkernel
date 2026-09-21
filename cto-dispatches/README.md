# CTO Dispatches

This directory is the CTO-to-coding-agent communication channel for Search Kernel.

A dispatch is a self-contained work order tied to this repository. It may cover
remediation, implementation, investigation, verification, migration, hardening,
cleanup, or documentation work.

## Contract

A dispatch should be specific enough that a coding agent can execute it without
redefining product semantics or inventing new acceptance criteria.

Each dispatch should normally include:

1. **Objective**
2. **Context / why this work exists**
3. **Authoritative files and frozen specs**
4. **Required changes**
5. **Regression tests first** when fixing a reported bug or correctness issue
6. **Verification commands and acceptance gates**
7. **Non-goals / forbidden shortcuts**
8. **Expected completion report**

## Rules

- The authoritative implementation contract remains in the normal Search Kernel
  specs, architecture, plans, and verifier fixtures. A dispatch points to those
  sources; it does not silently replace them.
- Keep scope bounded. One dispatch should correspond to one coherent unit of
  work.
- Do not use dispatches to smuggle in unrelated refactors.
- A coding agent must not weaken, delete, or rewrite a verifier merely to make a
  dispatch pass.
- For bug/correctness remediation, add the regression test first and prove it
  fails for the reported problem before changing production code.
- Completion means the requested implementation **and** its acceptance evidence
  are present.

## Naming

Use descriptive filenames that remain understandable in git history, for
example:

```text
typed-query-boundary-remediation.md
schema-fidelity-followup.md
m14-acceptance-verification.md
highlighting-client-parity.md
```

Dates are optional; prefer stable names over timestamp noise unless multiple
dispatches for the same topic genuinely need disambiguation.

## Lifecycle

Dispatches are checked into this repository so coding agents can read them next
to the code they modify.

Keep completed dispatches in git history for traceability. The current
management state still belongs in `superkelvint/searchkernel-cto-state`, not
here.
