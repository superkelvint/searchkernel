# Search Kernel clients

This directory contains the non-Rust language SDKs. The portable contract is
defined by the canonical FlatBuffers schemas in `api/*.fbs`.

Rust applications use the direct in-process Rust API. Non-Rust SDKs use this
path:

```text
ergonomic language-specific facade
        ↓
flatc-generated FlatBuffer models
        ↓
canonical request/response envelopes
        ↓
generated gRPC stub
        ↓
searchkerneld
```

The gRPC service is intentionally a small transport boundary:

```proto
message FlatBufferFrame {
    bytes payload = 1;
}

service SearchKernel {
    rpc Execute(FlatBufferFrame) returns (FlatBufferFrame);
}
```

Search operations are represented by the canonical FlatBuffers request union,
not by separate protobuf RPC methods.

## Ownership of the layers

FlatBuffers owns the request and response model, enums, unions, serialization,
deserialization, and wire compatibility.

gRPC tooling owns RPC framing, connection handling, client and server stub
generation, deadlines and cancellation, and UDS/TCP transport.

Each language SDK owns only idiomatic ergonomics: builders and helpers,
operator sugar, result convenience wrappers, and error presentation. SDKs do
not independently define Search Kernel defaults, semantic validation, ranking
or filter semantics, field capabilities, operation definitions, portable error
semantics, or full-schema serialization logic. Those remain centralized in the
canonical FlatBuffers-to-IR path:

```text
canonical FlatBuffers
        ↓
searchkernel-dispatch
        ↓
searchkernel-ir
```

The low-level complete API is the generated FlatBuffers object model. The
high-level ergonomic API is a small handwritten language facade over those
objects. A new FlatBuffers field is therefore immediately available through
the generated low-level model; an ergonomic facade can add convenient syntax
for it later.

## Python

The Python SDK currently follows this shape:

```text
Index / SearchBuilder / ergonomic helpers
        ↓
flatc-generated *T object classes
        ↓
generated Pack()/UnPack()
        ↓
GrpcTransport
        ↓
SearchKernel.Execute()
```

The generated Python models are the wire model. The SDK does not maintain a
parallel full-schema DTO layer or handwritten field-by-field FlatBuffer codec.

## Adding another language SDK

Before implementing a new language client, read
[`IMPLEMENTING_NEW_CLIENTS.md`](IMPLEMENTING_NEW_CLIENTS.md).

That document is the normative implementation SOP for new SDKs. It covers:

- generated FlatBuffers and gRPC layers;
- handwritten ergonomic facade boundaries;
- request/response framing and error handling;
- portable value encoding and decoding;
- managed local `searchkerneld` lifecycle where the host runtime supports it;
- browser vs native-runtime transport separation;
- packaging, generated-code reproducibility, and publish checks;
- cross-language conformance and end-to-end acceptance.

At a high level, every new first-class SDK should:

1. Generate the FlatBuffers models with `flatc`.
2. Generate or implement the standard gRPC transport from `api/rpc/searchkernel.proto`.
3. Implement the managed local `searchkerneld` lifecycle used by `Index.create(...)`
   and `Index.open(...)`: resolve the daemon binary, start it against the requested
   index, use a private local endpoint, wait for readiness, and own graceful shutdown
   and cleanup.
4. Reuse that same transport/client stack for `Index.connect(...)` when attaching to
   an already-running daemon.
5. Add the thin idiomatic facade for the language.
6. Run the shared protocol, semantic, packaging, managed-lifecycle, persistence/reopen,
   and end-to-end conformance tests.

Browser-only clients cannot spawn local processes, so they expose the connection path
only and talk to an already-running `searchkerneld` through the browser-compatible
transport. Native runtimes should provide the managed local-host behavior rather than
requiring users to launch the daemon manually.

Adding a language should not require changes to `searchkernel-ir`,
`searchkernel-dispatch`, `searchkerneld` protocol semantics, or Vespa lowering
unless the new language exposes a genuine protocol defect.
