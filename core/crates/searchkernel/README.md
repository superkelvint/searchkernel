# `searchkernel`

This is the product-facing Rust API. It provides the fluent schema, mutation,
filter, and search types that an application uses.

## Start reading here

1. `src/lib.rs` shows the public vocabulary and re-exports.
2. `src/schema.rs` shows index and field builders.
3. `src/index/mod.rs` shows the CRUD entry points and backend handoff.
4. `src/search.rs`, `src/filter.rs`, and `src/patch.rs` show request builders.
5. `src/portable_definition.rs` shows the narrow manifest-provenance encoder;
   request semantics live in `searchkernel-ir` resolvers.
6. `tests/api.rs` shows complete examples and expected boundary behavior.

## Key concepts

- Builders collect caller intent; they do not own search semantics.
- `Index` sends direct Rust resolver inputs to `searchkernel-ir`, which
  validates and normalizes them before handing owned IR to a backend. It does
  not serialize the request through FlatBuffers or gRPC.
- `Index::create` and `Index::open` use the native Vespa-backed lifecycle;
  `Index::from_backend` is test/infrastructure setup.
- Result documents are converted from owned backend values into caller-chosen
  `serde` types.

When changing this crate, preserve the public vocabulary and route every
operation through the core conversion path.
