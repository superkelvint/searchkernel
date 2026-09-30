# Search Kernel clients

This directory contains the non-Rust language SDKs. The portable contract is
defined by the canonical FlatBuffers schemas in `api/*.fbs`.

Rust applications use the direct in-process Rust API. Non-Rust SDKs use one
explicitly sanctioned portable transport path and converge on the same
dispatcher.

Default binary/native path:

```text
ergonomic language-specific facade
        ↓
flatc-generated FlatBuffer models
        ↓
canonical request/response envelope bytes
        ↓
generated gRPC/gRPC-Web stub
        ↓
searchkerneld
```

Explicit HTTP-first path:

```text
ergonomic language-specific facade
        ↓
standard FlatBuffers JSON envelope
        ↓
generic HTTP execute edge
        ↓
official BFBS-backed parser/printer
        ↓
canonical request/response envelope bytes
        ↓
searchkerneld shared dispatcher
```

## Capability status

Per-client support is not maintained as hand-edited booleans. The generated
[client capability matrix](../docs/CLIENT_CAPABILITIES.md) is derived from
`SKC:` evidence annotations colocated with executable tests and E2E examples.
The canonical capability IDs and backend stages remain in
[`docs/capabilities.json`](../docs/capabilities.json).

When adding client behavior, annotate the test that proves it rather than
editing the matrix. See
[`reference/client-contracts/README.md`](../reference/client-contracts/README.md).

Implementation/capability status is distinct from release support. The next beta's authoritative distribution scope is the [release support matrix](../docs/release-support.md): Python (managed-local/connect), TypeScript/Node.js (connect), and Ruby (connect-to-daemon) are release-qualified on Linux x86-64 glibc. Ruby managed-local, browser gRPC-Web, PHP, Go, and other platforms/runtimes remain deferred even when implementation or test-only lifecycle evidence exists.

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

For clients that use gRPC/gRPC-Web, standard gRPC tooling owns RPC framing,
connection handling, client/server stub generation, deadlines/cancellation, and
UDS/TCP or browser transport. Explicit HTTP-first clients do not reimplement
gRPC; they use standard FlatBuffers JSON and the generic HTTP execute edge.

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

Index lifecycle follows one canonical invariant across first-party clients: any client that exposes `CreateIndex` and `OpenIndex` also exposes `DeleteIndex`, with the logical target carried by the V2 request envelope. Managed-local clients may resolve a caller path to that logical target, but deletion still goes through the daemon lifecycle operation; closing a handle never deletes an index.

The low-level complete API is mechanically derived from the canonical
FlatBuffers contract. Binary clients expose generated FlatBuffers objects;
explicit HTTP-first clients expose supported standard-FlatBuffers-JSON
envelope/value/request helpers whose schema fidelity is checked through the
official BFBS bridge. The high-level ergonomic API is a small handwritten
facade over that low-level representation.

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

There are two client SOPs:

- [`IMPLEMENTING_NEW_CLIENTS.md`](IMPLEMENTING_NEW_CLIENTS.md) — how to build
  a correct first-party client.
- [`INTEGRATING_NEW_CLIENTS.md`](INTEGRATING_NEW_CLIENTS.md) — after the client
  works, how to wire it into repository-wide build, regeneration, conformance,
  capability, CI, packaging, and real-E2E verification so future drift cannot
  silently skip it.

Before implementing a new language client, read
[`IMPLEMENTING_NEW_CLIENTS.md`](IMPLEMENTING_NEW_CLIENTS.md), then complete
[`INTEGRATING_NEW_CLIENTS.md`](INTEGRATING_NEW_CLIENTS.md) before calling the
client first-class.

The implementation SOP covers:

- generated FlatBuffers/gRPC layers for binary clients and BFBS-backed standard-JSON conformance for HTTP-first clients;
- handwritten ergonomic facade boundaries;
- request/response framing and error handling;
- portable value encoding and decoding;
- managed local `searchkerneld` lifecycle where the host runtime supports it;
- browser vs native-runtime transport separation;
- packaging, generated-code reproducibility, and publish checks;
- cross-language conformance and end-to-end acceptance.

The integration SOP covers the repository promotion step, including:

- `./dev test <client>`, `./dev check`, `check-fast`, regeneration, and
  doctor wiring;
- client surface and runtime registration in the shared capability catalog;
- executable `SKC:` evidence and E2E-lane registration;
- shared conformance fixture consumption;
- generated capability/compatibility reports;
- CI bootstrap and workflow coverage;
- clean package-consumer verification;
- adversarial checks for false-green or silently omitted client lanes.

At a high level, every new first-class SDK should:

1. Select one architecture-sanctioned representation/transport path before implementation.
2. For binary clients, generate FlatBuffers models and standard gRPC/gRPC-Web plumbing; for explicitly HTTP-first clients, use only standard FlatBuffers JSON and prove it mechanically through the official BFBS bridge.
3. Implement managed local `searchkerneld` lifecycle only when that runtime claims it:
   resolve the daemon binary, confine it to a SearchKernel-owned managed data root,
   select the requested logical target through the canonical envelope, use a private local
   endpoint, wait for readiness, and own graceful shutdown and cleanup.
4. Reuse the same selected transport/client stack for connection to an already-running daemon.
5. Add the thin idiomatic facade for the language.
6. Integrate that client into the repository-wide generation/conformance, test,
   capability, packaging, and E2E harnesses.
7. Run the shared protocol, semantic, packaging, applicable lifecycle,
   persistence/reopen, and end-to-end conformance tests.

Browser-only clients cannot spawn local processes, so they expose the connection path
only and talk to an already-running `searchkerneld` through the browser-compatible
transport. Managed-local capability is explicit rather than universal: a connect-only
native or HTTP-first client must document that lifecycle boundary rather than
implicitly claiming daemon ownership.

Adding a language should not require changes to `searchkernel-ir`,
`searchkernel-dispatch`, `searchkerneld` protocol semantics, or Vespa lowering
unless the new language exposes a genuine protocol defect.
