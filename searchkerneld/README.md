# `searchkerneld`

`searchkerneld` is the local gRPC transport host for the canonical Search
Kernel FlatBuffers protocol.

Run it against an existing index with:

```text
searchkerneld --index /path/to/index --listen unix:///tmp/searchkerneld.sock
```

For a new index, pass its canonical FlatBuffers definition with `--definition`;
that option is only used during creation. On Unix, omitting `--listen` uses
`/tmp/searchkerneld.sock`. TCP addresses are also supported, for example
`--listen 127.0.0.1:50051`.

## Request and response path

The service contract intentionally consists of one generic operation:

```text
Execute(FlatBufferFrame) -> FlatBufferFrame
```

The actual Search Kernel operation is represented by the FlatBuffers request
union, so adding an operation normally requires FlatBuffers, IR, and
dispatcher work—not a gRPC service-schema change.

Requests follow this path:

```text
gRPC FlatBufferFrame
        ↓
payload bytes
        ↓
searchkernel-dispatch
        ↓
canonical RequestEnvelope
        ↓
*_to_ir()
        ↓
Search Kernel IR
        ↓
searchkernel-ir-adapter
        ↓
searchkernel-vespa-engine
        ↓
searchkernel-vespa-bindings
        ↓
Rust ↔ Vespa C++ native boundary
        ↓
Vespa
```

Responses take the inverse route:

```text
backend response
        ↓
canonical ResponseEnvelope
        ↓
FlatBuffer bytes
        ↓
gRPC FlatBufferFrame
```

The gRPC transport boundary is separate from the Rust ↔ Vespa C++ native
boundary. The former carries canonical FlatBuffer frames between processes;
the latter connects the Rust engine wrapper to Vespa.

## Responsibilities

`searchkerneld` owns gRPC server lifecycle, TCP and Unix-domain-socket
listening, request/response framing, connection handling, transport-level
errors, and server configuration. UDS is preferred for local same-host SDK
use where supported; TCP remains available for other deployments.

It does not independently implement query semantics, ranking semantics, filter
semantics, schema defaults, request validation, or operation-specific business
logic. Those belong to the canonical FlatBuffers, `searchkernel-dispatch`, and
`searchkernel-ir` path.

Where JSON is used, the rule is:

> JSON is the standard FlatBuffers JSON representation of the canonical schema. Search Kernel does not define a separate JSON API or custom JSON dialect.

The conversion path is:

```text
standard FlatBuffers JSON
        ↓
schema-aware FlatBuffers JSON parser
        ↓
canonical FlatBuffer bytes
        ↓
searchkernel-dispatch
```
