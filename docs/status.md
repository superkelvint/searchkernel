# Current status

This is the current-state document for Search Kernel. It answers “what is in
the repository now?” and is intentionally separate from the architecture,
roadmap, and history documents.

**Snapshot:** 2026-09-20

Status in this document means that the implementation, contract, or test
harness is present in the repository. It does not mean that an acceptance
verifier has been run successfully in every environment. Use the linked
verifier when a milestone must be declared accepted.

## Read the right document

| Question | Document |
| --- | --- |
| What must always be true? | [Architecture](architecture.md) |
| What do we intend to build? | [Roadmap](roadmap.md) |
| What is true in the current tree? | **Current status** |
| Does the current tree support a capability? | [Capability ledger](CAPABILITIES.md) |
| Why are old paths and artifacts still here? | [History](history.md) |

## Current architecture

The supported product paths are:

```text
Rust application
    -> direct Rust API
    -> Search Kernel IR
    -> Vespa adapter and native engine

Non-Rust application
    -> generated FlatBuffers models
    -> canonical RequestEnvelope bytes
    -> generated gRPC client
    -> searchkerneld
    -> shared dispatcher and Search Kernel IR
    -> Vespa adapter and native engine
```

FlatBuffers is the portable domain contract. gRPC carries opaque canonical
FlatBuffer frames. The Rust product API remains in-process and does not route
through gRPC.

## Implementation inventory

| Area | Current state | Evidence |
| --- | --- | --- |
| Native Rust/Vespa wrapper | Existing M0–M11 foundation; still the required embedded path | [`core/crates/`](../core/crates/), [`core/native/vespa/`](../core/native/vespa/) |
| M12 dense query tensors and nearest-neighbor execution | Contract and implementation are present; acceptance is cumulative on M11 | [`reference/vespa/m12/`](../reference/vespa/m12/), [`reference/vespa/m12/verify-m12.sh`](../reference/vespa/m12/verify-m12.sh) |
| M13 dense tensor document CRUD | Contract and implementation are present; acceptance is cumulative on M12 | [`reference/vespa/m13/`](../reference/vespa/m13/), [`reference/vespa/m13/verify-m13.sh`](../reference/vespa/m13/verify-m13.sh) |
| Portable FlatBuffers contract and IR | Active semantic path used by Rust and non-Rust requests | [`api/searchkernel.fbs`](../api/searchkernel.fbs), [`core/crates/searchkernel-ir/`](../core/crates/searchkernel-ir/), [`core/crates/searchkernel-dispatch/`](../core/crates/searchkernel-dispatch/) |
| gRPC daemon | Present; generic FlatBuffer-frame service with UDS and TCP listeners | [`searchkerneld/`](../searchkerneld/), [`api/rpc/searchkernel.proto`](../api/rpc/searchkernel.proto) |
| Python transport | Present; `GrpcTransport` uses generated gRPC bindings and canonical frames | [`clients/python/searchkernel/grpc_transport.py`](../clients/python/searchkernel/grpc_transport.py), [`clients/python/tests/test_grpc_transport.py`](../clients/python/tests/test_grpc_transport.py) |
| Python ergonomic facade | Present on top of the gRPC client, including managed local daemon mode | [`clients/python/searchkernel/client.py`](../clients/python/searchkernel/client.py), [`clients/python/searchkernel/ergonomic.py`](../clients/python/searchkernel/ergonomic.py), [`clients/python/tests/test_grpc_e2e.py`](../clients/python/tests/test_grpc_e2e.py) |
| Retired portable client ABI | No longer a supported product path | [History](history.md) |

The repository therefore no longer has a meaningful “M11 is the final
milestone” status. M12 and M13 extend the low-level Vespa track, while the
gRPC daemon and Python migration have moved from planned work into the current
implementation.

## Verification state

The checked-in contracts and verifier entry points are the acceptance
authority. Keep these facts separate from implementation presence:

- M12 acceptance: [`reference/vespa/m12/verify-m12.sh`](../reference/vespa/m12/verify-m12.sh).
- M13 acceptance: [`reference/vespa/m13/verify-m13.sh`](../reference/vespa/m13/verify-m13.sh); it invokes M12 cumulatively.
- Daemon transport tests: [`searchkerneld/tests/grpc.rs`](../searchkerneld/tests/grpc.rs).
- Python gRPC and facade tests: [`clients/python/tests/`](../clients/python/tests/).

When reporting a milestone, state both the implementation state and the
verification command/result. Do not infer a passing verifier merely from the
presence of its directory or contract.

## Immediate roadmap orientation

The next work should be selected from the actual gaps, not from the old
“M11-final / C7-C8-future” narrative:

1. Keep M12 and M13 cumulative acceptance green as the low-level tensor path
   expands.
2. Complete the remaining real-engine integration and cross-entry-point
   conformance work described by the I-series.
3. Treat additional language facades and broader tensor/vector capability as
   planned work unless their implementation and acceptance evidence appear in
   the repository.

This section is directional only. The implementation inventory above is the
source of current status.
