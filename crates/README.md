# Rust crates

The Rust workspace is split by responsibility. Read the crates in this order:

1. `searchkernel` — the product-facing fluent API.
2. `searchkernel-core` — validation, defaults, and the owned intermediate
   representation.
3. `searchkernel-testkit` — backend-free recording, scripting, and conformance
   support.
4. `searchkernel-vespa` — the Vespa-specific adapter and linguistics/config
   replacement code.
5. `searchkernel-vespa-sys` — raw generated bindings for the private native
   ABI.
6. `searchkernel-ffi` — the portable C ABI transport layer.
7. `searchkernel-api` — generated FlatBuffers Rust types.

The normal data flow is:

```text
public Rust API
    -> FlatBuffers contract
    -> owned Search Kernel IR
    -> backend
    -> Vespa lowering/native execution
```

Keep public API code free of Vespa types, keep raw `unsafe` in the sys/FFI
boundaries, and keep Vespa behavior in the Vespa adapter or native engine.
