# `searchkernel-testkit`

This crate provides backend-free tools for testing the public API and core
boundary. It records requests, replays exact scripted calls, and runs frozen
JSON conformance corpora.

## Start reading here

1. `src/lib.rs` shows the exported test helpers.
2. `src/recording.rs` shows the simplest backend: record and fail.
3. `src/scripted.rs` shows exact ordered request/reply replay.
4. `src/runner.rs` shows the conformance-corpus pipeline.
5. `tests/backends.rs` and `tests/conversions.rs` show intended use.

## Key concepts

- The testkit does not implement matching, ranking, or storage.
- A recording backend proves what IR the facade produced, not that a search
  engine executed it.
- A scripted backend compares operation kind and complete owned request before
  returning a typed result.
- Frozen corpus goldens are acceptance inputs, not outputs to regenerate from
  the implementation.
