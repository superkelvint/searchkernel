# Search Kernel core

`core/` is the embeddable Search Kernel product boundary. It is an
organizational directory, not a Cargo package; the implementation remains
split into small crates so the semantic, transport, and Vespa dependency
firewalls stay explicit.

The canonical Portable API lives outside this directory in `api/`. Generated
Rust API types, codecs, the native ABI, the safe Vespa engine wrapper, and the
backend adapter live under `core/crates/`.

The direct Rust product path is:

```text
public Rust API
    -> searchkernel-ir resolver inputs
    -> searchkernel-ir
    -> searchkernel-ir-adapter
    -> searchkernel-vespa-engine
    -> searchkernel-vespa-bindings
```

Portable clients use the separate transport path before converging on the same
IR and backend layers:

```text
FlatBuffers bytes
    -> searchkernel-dispatch
    -> searchkernel-ir
    -> searchkernel-ir-adapter
    -> searchkernel-vespa-engine
    -> searchkernel-vespa-bindings
```

The optional Rust remote client (`searchkernel` feature `remote`) uses this
canonical transport path while preserving the public `Index` facade. Plain
`host:port` and `http://` endpoints use plaintext gRPC, and the built-in
client does not add TLS or authentication credentials. Treat that TCP endpoint
as a trusted-network boundary: use a private network, tunnel, or service mesh
when transport security or authentication is required, and do not expose it
directly to an untrusted Internet. On Unix platforms, `unix:///path` is
available when a local Unix-domain socket is the appropriate deployment
boundary.

`searchkernel-protocol::json` is a small generic boundary around the official
schema-aware FlatBuffers JSON parser/printer; it contains no Search Kernel
field mappings. `searchkernel-dispatch` is the canonical bytes-to-IR routing
path. Native adapter sources and the generated SDK are under `core/native/`.
