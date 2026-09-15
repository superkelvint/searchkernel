# `searchkernel`

This is the product-facing Rust API. It provides the fluent schema, mutation,
filter, and search types that an application uses.

## Start reading here

1. `src/lib.rs` shows the public vocabulary and re-exports.
2. `src/schema.rs` shows index and field builders.
3. `src/index.rs` shows the CRUD entry points and backend handoff.
4. `src/search.rs`, `src/filter.rs`, and `src/patch.rs` show request builders.
5. `src/wire.rs` shows how those builders become the portable contract.
6. `tests/api.rs` shows complete examples and expected boundary behavior.

## Key concepts

- Builders collect caller intent; they do not own search semantics.
- `Index` serializes a request, asks `searchkernel-ir` to validate and
  normalize it, then sends owned IR to a backend.
- `Index::from_backend` is test/infrastructure setup. The production native
  lifecycle is supplied by the Vespa integration milestone.
- Result documents are converted from owned backend values into caller-chosen
  `serde` types.

When changing this crate, preserve the public vocabulary and route every
operation through the core conversion path.
