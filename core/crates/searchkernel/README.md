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
- `Index::create` and `Index::open` use the native Vespa-backed lifecycle.
- `RemoteClient::connect` connects to a multi-index daemon; its lifecycle
  methods use canonical requests, and returned `Index` handles stay bound to
  their selected target.
- `Index::from_backend` is test/infrastructure setup.
- Result documents are converted from owned backend values into caller-chosen
  `serde` types.

Remote use selects a target through the daemon client:

```rust
let client = RemoteClient::connect("127.0.0.1:50051")?;
let products = client.create_index("products", definition)?;
products.insert("sku-1", &product)?;
products.close()?;

let products = client.open_index("products")?;
let results = products.search("keyboard").execute::<Product>()?;
client.delete_index("products")?;
```

`Index::connect(endpoint)` is deprecated because an endpoint alone does not
select a logical index. Use `RemoteClient::connect(endpoint)` followed by
`create_index(target, definition)` or `open_index(target)`.

Search result fields are also available as `ResultValue`, which preserves the
portable value kinds, including unsigned integers, bytes, tensors, and explicit
NaN or infinity values. `Hit<T>::document` is `None` when the lossless
fields cannot pass through the JSON-backed document projection (for example,
because they contain non-finite scalars). Inspect `Hit<T>::fields` for the
lossless values and `Hit<T>::document_error` for that fallback reason.
Ordinary serde deserialization failures for the caller's `T` remain errors.
Portable non-double/non-byte feature values use `FeatureValue::Value(ResultValue)` rather than a JSON-specific feature variant, keeping one lossless result vocabulary across the Rust API.

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
