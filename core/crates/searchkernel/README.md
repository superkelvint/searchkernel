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

Search result fields are also available as `ResultValue`, which preserves the
portable value kinds, including unsigned integers, bytes, tensors, and explicit
NaN or infinity values. `Hit<T>::document` is `None` when the lossless
fields cannot pass through the JSON-backed document projection (for example,
because they contain non-finite scalars). Inspect `Hit<T>::fields` for the
lossless values and `Hit<T>::document_error` for that fallback reason.
Ordinary serde deserialization failures for the caller's `T` remain errors.

For example, a non-finite field is read without a JSON conversion:

```rust
match hit.fields.get("score") {
    Some(ResultValue::NonFiniteFloat(NonFiniteFloat::PositiveInfinity)) => { /* ... */ }
    Some(ResultValue::Float64(score)) => { /* finite score */ }
    _ => { /* absent or another value kind */ }
}
```

When changing this crate, preserve the public vocabulary and route every
operation through the core conversion path.
