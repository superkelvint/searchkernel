# Rust crates

The Rust workspace is split by responsibility. The crates currently live here:

- `searchkernel` — the product-facing fluent API.
- `searchkernel-dispatch` — transport-independent canonical request dispatch.
- `searchkernel-ir` — validation, defaults, and the owned intermediate
  representation.
- `searchkernel-ir-adapter` — the integration boundary between IR and Vespa.
- `searchkernel-protocol::json` — generic standard FlatBuffers JSON conversion.
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
`searchkernel-vespa-bindings` for the direct Rust/native path. Read
`searchkernel-protocol` and `searchkernel-dispatch` when following a portable
FlatBuffers or gRPC request.

The direct Rust data flow is:

```text
public Rust API
    -> SearchIntent / resolver input
    -> owned Search Kernel IR
    -> searchkernel-ir-adapter
    -> Vespa lowering/native execution
```

The portable data flow is:

```text
FlatBuffers request
    -> searchkernel-dispatch
    -> owned Search Kernel IR
    -> searchkernel-ir-adapter
    -> Vespa lowering/native execution
```

The Rust API does not serialize through FlatBuffers or gRPC merely to reach
the in-process engine. Both paths share the owned IR and backend boundary.

Keep public API code free of Vespa types, keep raw `unsafe` in the bindings/FFI
boundaries, and keep Vespa behavior in the Vespa engine integration or native
adapter.
