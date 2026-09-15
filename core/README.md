# Search Kernel core

`core/` is the embeddable Search Kernel product boundary. It is an
organizational directory, not a Cargo package; the implementation remains
split into small crates so the semantic, transport, and Vespa dependency
firewalls stay explicit.

The canonical Portable API lives outside this directory in `api/`. Generated
Rust API types, codecs, the native ABI, the safe Vespa engine wrapper, and the
backend adapter live under `core/crates/`.

```text
api/*.fbs
    -> searchkernel-protocol
    -> searchkernel-ir
    -> searchkernel-ir-adapter
    -> searchkernel-vespa-engine
    -> searchkernel-vespa-bindings
```

The portable JSON and client crates (`searchkernel-json` and
`searchkernel-client`) also live under `core/crates/`. Native adapter sources
and the generated SDK are under `core/native/`.
