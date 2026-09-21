# 14. Python client — `clients/python`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + GEN + PKG`; every capability claimed functional to Python requires production-transport `E2E`.

## 14.1 Generated FlatBuffers/gRPC artifacts and package bootstrap

- [ ] **SK-14-001** Python FlatBuffers generated code matches the current canonical `.fbs` files.
- [ ] **SK-14-002** `_wire_generated` and `rpc/searchkernel_pb2*.py` are either shipped as package source or regenerated before build/install; stale local `*.egg-info`/cache state cannot make an incomplete source tree appear healthy.
- [ ] **SK-14-003** Python gRPC stubs match the current `.proto`.
- [ ] **SK-14-004** Runtime dependency versions are compatible with the versions used to generate code.
- [ ] **SK-14-005** Clean package import succeeds in a fresh environment using declared dependencies only.
- [ ] **SK-14-006** Generation/check scripts fail on stale artifacts.

## 14.2 Protocol/value encoding — `protocol.py`

- [ ] **SK-14-007** Python bool/int/float/string/bytes/list/object values encode to the intended canonical union variants.
- [ ] **SK-14-008** Int64/UInt64 limits are enforced without loss.
- [ ] **SK-14-009** Integer-valued floats do not accidentally change type unless explicitly requested by facade rules.
- [ ] **SK-14-010** Dense/sparse/mixed tensor values encode/decode without changing tensor kind.
- [ ] **SK-14-011** Non-finite values preserve their explicit portable representation.
- [ ] **SK-14-012** Bytes are distinguished by discriminator, not inferred from integer-list contents.
- [ ] **SK-14-013** Response values decode by union discriminator and reject unknown variants.

## 14.3 Ergonomic schema/query/grouping builders — `ergonomic.py`

- [ ] **SK-14-014** Builders collect intent only and do not reimplement central semantic validation.
- [ ] **SK-14-015** Explicit zero/false/empty values are preserved; Python truthiness (`x or default`) never overwrites valid zero values.
- [ ] **SK-14-016** `facet(limit=0)` and similar zero-valued options preserve zero.
- [ ] **SK-14-017** Pagination validates uint32/range/overflow semantics before encoding.
- [ ] **SK-14-018** Typed query helpers emit the intended canonical generated objects.
- [ ] **SK-14-019** Grouping helpers preserve aliases, nested structure, aggregations, and summary aggregation.
- [ ] **SK-14-020** Unsupported client sugar does not imply backend support.
- [ ] **SK-14-021** YQL builder does not advertise incompatible high-level methods, or rejects them locally in a deterministic way consistent with the facade contract.
- [ ] **SK-14-022** Analyze requires a valid target and exposes field/analyzer choice as specified.

## 14.4 Result decoding — `client.py` / result wrappers

- [ ] **SK-14-023** Canonical root `ResultNode` tree is decoded recursively and losslessly.
- [ ] **SK-14-024** Unknown result-node union variants fail closed.
- [ ] **SK-14-025** Nested Hit and ResultGroup ordering is preserved.
- [ ] **SK-14-026** Positive/negative infinite relevance decode to intentional language values, not generated-table objects/null.
- [ ] **SK-14-027** Hit fields/features/highlights/sort values preserve portable types.
- [ ] **SK-14-028** Sparse/mixed tensor responses are not mislabeled as dense.
- [ ] **SK-14-029** Lists of integers remain lists unless the source discriminator is `BytesValue`.
- [ ] **SK-14-030** Group continuations remain opaque strings.
- [ ] **SK-14-031** Facet/aggregation convenience projections are derived only from recognized canonical grouping shapes.
- [ ] **SK-14-032** Operation errors and result-embedded errors remain distinct structured objects.

## 14.5 gRPC transport — `grpc_transport.py`

- [ ] **SK-14-033** Uses the standard generated Python gRPC stub; no custom gRPC wire/framing implementation exists.
- [ ] **SK-14-034** UDS and TCP targets are formed correctly.
- [ ] **SK-14-035** Request bytes are passed unchanged inside `FlatBufferFrame`.
- [ ] **SK-14-036** gRPC deadlines/cancellation/status errors map appropriately without swallowing portable payload errors.
- [ ] **SK-14-037** Transport does not parse/validate Search Kernel semantics.

## 14.6 Managed local host

- [ ] **SK-14-038** `Index.create/open` daemon spawning uses unique/private socket paths.
- [ ] **SK-14-039** Spawn failure/timeout cleans child process/socket/temp state.
- [ ] **SK-14-040** Client close terminates only the daemon instance it owns.
- [ ] **SK-14-041** Crash/abnormal child exit is detected and surfaced.
- [ ] **SK-14-042** Multiple local clients cannot unlink/kill one another's daemon/socket.
- [ ] **SK-14-043** Explicit daemon binary override and repository fallback behave deterministically.

## 14.7 Python E2E

- [ ] **SK-14-044** Real-engine E2E runs through the production gRPC path, not a recording/scripted backend.
- [ ] **SK-14-045** CRUD, patch, advanced update, get/delete, and reopen are covered.
- [ ] **SK-14-046** Text search, textual filters/bindings, repeated clauses, sort, pagination, projection, total hits are covered.
- [ ] **SK-14-047** Facets and root aggregations are covered.
- [ ] **SK-14-048** Nested grouping, aggregation order/precision/aliases, summary aggregation, and recursive result decoding are covered.
- [ ] **SK-14-049** Typed queries cover all Python-advertised variants.
- [ ] **SK-14-050** Ranking profile + rank properties + query inputs + two-phase scoring are covered where supported.
- [ ] **SK-14-051** Tensor/NN path is covered, including approximate ANN once executable.
- [ ] **SK-14-052** YQL property substitution and valid request-level combinations are covered.
- [ ] **SK-14-053** Structured operation errors and result errors are covered.
- [ ] **SK-14-054** Clean package/import test is part of the authoritative client verification gate.

---
