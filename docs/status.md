# Current status

This is the current-state document for Search Kernel. It answers “what is in
the repository now?” and is intentionally separate from the architecture,
roadmap, and history documents.

**Snapshot:** 2026-09-24

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

Non-Rust binary/native application
    -> generated FlatBuffers models
    -> canonical RequestEnvelope bytes
    -> generated gRPC/gRPC-Web client
    -> searchkerneld
    -> shared dispatcher and Search Kernel IR
    -> Vespa adapter and native engine

Explicit HTTP-first application
    -> standard FlatBuffers JSON RequestEnvelope
    -> searchkerneld generic HTTP execute edge
    -> official BFBS-backed FlatBuffers JSON parser
    -> canonical RequestEnvelope bytes
    -> the same shared dispatcher and Search Kernel IR
    -> Vespa adapter and native engine
```

FlatBuffers is the portable domain contract. gRPC/gRPC-Web carries opaque
canonical FlatBuffer frames for binary clients; explicitly designated HTTP-first
clients carry only standard FlatBuffers JSON through the official BFBS bridge.
The Rust product API remains in-process and does not route through either
portable transport.

## Implementation inventory

| Area | Current state | Evidence |
| --- | --- | --- |
| Native Rust/Vespa wrapper | Existing M0–M11 foundation; still the required embedded path | [`core/crates/`](../core/crates/), [`core/native/vespa/`](../core/native/vespa/) |
| M12 dense query tensors and nearest-neighbor execution | Contract and implementation are present; acceptance is cumulative on M11 | [`reference/vespa/m12/`](../reference/vespa/m12/), [`reference/vespa/m12/verify-m12.sh`](../reference/vespa/m12/verify-m12.sh) |
| M13 dense tensor document CRUD | Contract and implementation are present; acceptance is cumulative on M12 | [`reference/vespa/m13/`](../reference/vespa/m13/), [`reference/vespa/m13/verify-m13.sh`](../reference/vespa/m13/verify-m13.sh) |
| Portable FlatBuffers contract and IR | Active semantic path used by Rust and non-Rust requests | [`api/searchkernel.fbs`](../api/searchkernel.fbs), [`core/crates/searchkernel-ir/`](../core/crates/searchkernel-ir/), [`core/crates/searchkernel-dispatch/`](../core/crates/searchkernel-dispatch/) |
| Daemon portable transports | Present; generated gRPC service over UDS/TCP, gRPC-Web, and a generic HTTP standard-FlatBuffers-JSON execute edge all terminate at the shared dispatcher | [`searchkerneld/`](../searchkerneld/), [`api/rpc/searchkernel.proto`](../api/rpc/searchkernel.proto), [Architecture](architecture.md#non-rust-languages) |
| Python transport | Present; `GrpcTransport` uses generated gRPC bindings and canonical frames | [`clients/python/searchkernel/grpc_transport.py`](../clients/python/searchkernel/grpc_transport.py), [`clients/python/tests/test_grpc_transport.py`](../clients/python/tests/test_grpc_transport.py) |
| Python ergonomic facade | Present on top of the gRPC client, including managed local daemon mode | [`clients/python/searchkernel/client.py`](../clients/python/searchkernel/client.py), [`clients/python/searchkernel/ergonomic.py`](../clients/python/searchkernel/ergonomic.py), [`clients/python/tests/test_grpc_e2e.py`](../clients/python/tests/test_grpc_e2e.py) |
| TypeScript transport/facade | Present; Node uses the standard generated gRPC client, browser transport uses generated gRPC-Web, and the shared facade has a real-engine Node E2E lane | [`clients/typescript/src/transport/grpc-node.ts`](../clients/typescript/src/transport/grpc-node.ts), [`clients/typescript/src/transport/grpc-web.ts`](../clients/typescript/src/transport/grpc-web.ts), [`clients/typescript/examples/try_searchkernel_client_api_e2e.ts`](../clients/typescript/examples/try_searchkernel_client_api_e2e.ts) |
| PHP transport/facade | Present as an explicitly HTTP-first client; it emits standard FlatBuffers JSON to `POST /v1/execute`, which is converted by the BFBS-backed bridge before the shared dispatcher | [`clients/php/src/HttpTransport.php`](../clients/php/src/HttpTransport.php), [`clients/php/src/Index.php`](../clients/php/src/Index.php), [`clients/php/tests/`](../clients/php/tests/) |
| Retired portable client ABI | No longer a supported product path | [History](history.md) |

The repository therefore no longer has a meaningful “M11 is the final
milestone” status. M12 and M13 extend the low-level Vespa track, while the
daemon plus Python, TypeScript, and PHP client paths are present implementation
surfaces. Transport uniformity is not the contract: every first-party client
must use its explicitly sanctioned representation path and converge on the same
canonical FlatBuffer dispatcher/IR semantics.

## Verification state

The checked-in contracts and verifier entry points are the acceptance
authority. Keep these facts separate from implementation presence:

- M12 acceptance: [`reference/vespa/m12/verify-m12.sh`](../reference/vespa/m12/verify-m12.sh).
- M13 acceptance: [`reference/vespa/m13/verify-m13.sh`](../reference/vespa/m13/verify-m13.sh); it invokes M12 cumulatively.
- Daemon transport tests: [`searchkerneld/tests/`](../searchkerneld/tests/).
- Python gRPC and facade tests: [`clients/python/tests/`](../clients/python/tests/).
- TypeScript generated gRPC/gRPC-Web, package, facade, and E2E evidence: [`clients/typescript/tests/`](../clients/typescript/tests/), [`clients/typescript/examples/`](../clients/typescript/examples/).
- PHP standard-FlatBuffers-JSON/BFBS, facade, package, and E2E evidence: [`clients/php/tests/`](../clients/php/tests/), [`clients/php/examples/`](../clients/php/examples/).

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
3. Treat additional language facades beyond the clients actually present in
   the tree, and broader tensor/vector capability, as planned work unless their
   implementation and acceptance evidence appear in the repository.

This section is directional only. The implementation inventory above is the
source of current status.
