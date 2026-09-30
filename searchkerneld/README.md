# `searchkerneld`

`searchkerneld` is the local gRPC transport host for the canonical Search
Kernel FlatBuffers protocol.

Start one daemon for a data root that can own multiple logical indexes:

```text
searchkerneld --data-root /var/lib/searchkernel \
  --listen unix:///tmp/searchkerneld.sock
```

Clients create and open indexes with canonical `CreateIndexRequest` and
`OpenIndexRequest` envelopes. The schema belongs to the create request, and
every ordinary request carries the selected logical `IndexTarget`. The daemon
owns physical index locations beneath `--data-root`.

On Unix, omitting `--listen` uses `/tmp/searchkerneld.sock`. TCP addresses are
also supported, for example `--listen 127.0.0.1:50051`.

To expose the HTTP/1 edge, add `--web-listen`. The same listener serves
browser gRPC-Web clients and the standard FlatBuffers JSON endpoint used by the
first HTTP clients:

```text
searchkerneld --data-root /var/lib/searchkernel \
  --listen unix:///tmp/searchkerneld.sock \
  --web-listen 127.0.0.1:50052
```

The HTTP/JSON operation is deliberately generic:

```text
POST /v1/execute
Content-Type: application/json

standard FlatBuffers JSON RequestEnvelope
    ->
standard FlatBuffers JSON ResponseEnvelope
```

The JSON body is not a second Search Kernel domain schema. `searchkerneld`
converts it through the existing BFBS-backed FlatBuffers JSON parser, dispatches
the resulting canonical FlatBuffer bytes through `searchkernel-dispatch`, and
prints the canonical response with the matching FlatBuffers JSON printer.

The HTTP/1 listener is optional and separate from the local listener. Both
listeners share the same dispatcher, admission limit, and bounded execution
limit. The `SEARCHKERNEL_GRPC_WEB_ADDR` environment variable is equivalent to
`--web-listen` when the flag is omitted.

If a Unix socket path already exists, startup first probes it. A live server
causes startup to fail; an unreachable stale socket is removed before binding.

Native dispatches are admitted through a bounded blocking executor. The
default limit is 16 concurrent dispatches; override it with
`--execution-limit N` or `SEARCHKERNEL_EXECUTION_LIMIT`. Requests above the
limit wait asynchronously until a dispatch slot is available.

The transport also bounds total admitted requests before FlatBuffer payload
decoding. The default admission limit is 32 requests across both listeners;
override it with `--admission-limit N` or
`SEARCHKERNEL_ADMISSION_LIMIT`. Requests above that bound are rejected with
`ResourceExhausted` (or the equivalent gRPC-Web trailer) instead of being
queued with decoded payload memory.

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

The Rust gRPC module is checked in at `src/generated/rpc.rs`. Regenerate it
with `tools/generate-searchkernel-rpc.sh`; `tools/check-searchkernel-rpc.sh`
compares fresh standard-tool output with the checked-in file.

## Responsibilities

`searchkerneld` owns gRPC and gRPC-Web server lifecycle, TCP and
Unix-domain-socket listening, request/response framing, connection handling,
transport-level errors, and server configuration. UDS is preferred for local
same-host SDK use where supported; the optional HTTP/1 TCP listener is for
browser gRPC-Web clients.

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
