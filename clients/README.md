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

1. Generate the FlatBuffers models with `flatc`.
2. Generate the standard gRPC client from `api/rpc/searchkernel.proto`.
3. Add a thin idiomatic facade where useful.
4. Run the shared protocol and semantic conformance tests.

Adding a language should not require changes to `searchkernel-ir`,
`searchkernel-dispatch`, `searchkerneld` protocol semantics, or Vespa lowering
unless the new language exposes a genuine protocol defect.
