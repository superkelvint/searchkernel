# Rust crates

The Rust workspace is split by responsibility. The crates currently live here:

- `searchkernel` — the product-facing fluent API.
- `searchkernel-dispatch` — transport-independent canonical V2 request dispatch.
- `searchkernel-ir` — validation, defaults, and the owned intermediate
  representation.
- `searchkernel-ir-adapter` — the integration boundary between IR and Vespa.
- `searchkernel-json` — FlatBuffers JSON conversion and generated JSON support.
- `searchkernel-protocol` — generated FlatBuffers Rust types.
- `searchkernel-testkit` — backend-free recording, scripting, and conformance
  support.
- `searchkernel-vespa-engine` — the Vespa-specific adapter and linguistics/config
  replacement code.
- `searchkernel-vespa-bindings` — raw generated bindings for the private native
  ABI.

The workspace manifest is at the repository root (`Cargo.toml`); this directory
contains its embeddable crates, while `tools/` contains the separate generator
crate.

For a guided reading order, start with `searchkernel`, then follow
`searchkernel-ir`, `searchkernel-ir-adapter`, `searchkernel-vespa-engine`, and
`searchkernel-vespa-bindings` for the native path.

The normal data flow is:

```text
public Rust API
    -> FlatBuffers contract
    -> owned Search Kernel IR
    -> backend
    -> Vespa lowering/native execution
```

Keep public API code free of Vespa types, keep raw `unsafe` in the bindings/FFI
boundaries, and keep Vespa behavior in the Vespa engine integration or native
adapter.
