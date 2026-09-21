# 15. TypeScript client — `clients/typescript`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + GEN + PKG`; every capability claimed functional to TypeScript requires production-transport `E2E`.

## 15.1 Generated artifacts and package bootstrap

- [ ] **SK-15-001** FlatBuffers TypeScript generated sources exist or are generated automatically before every clean build/package; `tsc` imports from `src/generated/**` must never rely on a developer-only ignored tree.
- [ ] **SK-15-002** gRPC/gRPC-Web generated sources exist or are generated automatically before every clean build/package.
- [ ] **SK-15-003** `npm run generate` is deterministic from canonical schemas/proto.
- [ ] **SK-15-004** `npm run build` works from a clean checkout with no pre-existing generated tree.
- [ ] **SK-15-005** `npm run test:package` validates the actual packed package, not workspace-relative source fallbacks.
- [ ] **SK-15-006** FlatBuffers runtime version matches the pinned generation/toolchain contract exactly enough to be supported; this snapshot's package manifest must be checked specifically because generation is pinned to `25.12.19` while the declared npm runtime is `25.9.23`.
- [ ] **SK-15-007** Generated protobuf/gRPC-Web code is produced by upstream tooling only; generated message classes do **not** justify a handwritten Node HTTP/2 gRPC transport.

## 15.2 Low-level/generated API — `low-level.ts`, `protocol/envelope.ts`

- [ ] **SK-15-008** Low-level exports expose generated wire types only; they do not create a second semantic implementation.
- [ ] **SK-15-009** Envelope construction/verification uses correct root/version/discriminators.
- [ ] **SK-15-010** Unknown/malformed response payloads fail closed.

## 15.3 Scalar/value layer — `internal/scalars.ts`, `client/values.ts`

- [ ] **SK-15-011** Safe JS integers encode without loss; unsafe integers require bigint/string helpers.
- [ ] **SK-15-012** `int64`/`uint64` helpers enforce exact signed/unsigned ranges.
- [ ] **SK-15-013** Explicit `float64()` preserves integer-valued floats as float values.
- [ ] **SK-15-014** Bytes and integer arrays remain distinct.
- [ ] **SK-15-015** Dense/sparse/mixed tensors encode/decode losslessly.
- [ ] **SK-15-016** Non-finite relevance/value representations map intentionally.

## 15.4 Ergonomic schema/query/grouping — `client/index.ts`, `ergonomic/{builders,query,grouping,schema}.ts`

- [ ] **SK-15-017** Builders only translate syntax sugar to canonical generated requests.
- [ ] **SK-15-018** JavaScript truthiness never replaces explicit `0`, `false`, or empty values with defaults; use nullish semantics where appropriate.
- [ ] **SK-15-019** `facet(limit=0)` remains zero.
- [ ] **SK-15-020** Pagination/page math checks uint32 overflow and invalid combinations.
- [ ] **SK-15-021** Query DSL covers only claimed variants and preserves annotations.
- [ ] **SK-15-022** Grouping helpers preserve recursive shape, aliases, functions, and aggregations.
- [ ] **SK-15-023** Schema helpers preserve intended field/rank-profile/tensor definition data.
- [ ] **SK-15-024** Rank-profile ergonomics are audited field-by-field. In this snapshot `RankProfile` exposes first phase, match features, and query inputs but no second-phase/rerank-count sugar; that is recorded as an ergonomic gap rather than mistaken for loss of low-level representability.
- [ ] **SK-15-025** YQL-incompatible methods are not exposed or are deterministically rejected.

## 15.5 Result decoder — `client/results.ts`

- [ ] **SK-15-026** Recursive heterogeneous result tree is lossless.
- [ ] **SK-15-027** Unknown union variants fail closed.
- [ ] **SK-15-028** Infinite relevance maps to `Infinity` / `-Infinity` intentionally.
- [ ] **SK-15-029** Int64/UInt64 decode to bigint without narrowing.
- [ ] **SK-15-030** Dense/sparse/mixed tensor values retain their kind and addresses/blocks.
- [ ] **SK-15-031** Hit/group features/metadata/continuations/ordering are preserved.
- [ ] **SK-15-032** Convenience facets/aggregations do not mutate or discard the canonical tree.

## 15.6 Browser gRPC-Web transport — `transport/grpc-web.ts`

- [ ] **SK-15-033** Uses standard generated gRPC-Web/client tooling.
- [ ] **SK-15-034** Does not implement Search Kernel domain semantics.
- [ ] **SK-15-035** Frame payload bytes remain canonical FlatBuffers bytes.
- [ ] **SK-15-036** Transport errors are distinct from portable errors.
- [ ] **SK-15-037** Cancellation/deadline behavior is tested.

## 15.7 Node transport — `transport/grpc-node.ts`

- [ ] **SK-15-038** Node uses a standard generated gRPC client implementation/stub.
- [ ] **SK-15-039** There is **no hand-written HTTP/2 gRPC framing, 5-byte frame parsing, trailer/status decoding, compression handling, or custom grpc-timeout implementation**.
- [ ] **SK-15-040** UDS and TCP are configured through standard client-channel facilities.
- [ ] **SK-15-041** Unary response cardinality/framing/compression are delegated to the gRPC library.
- [ ] **SK-15-042** Cancellation/deadlines use standard gRPC mechanisms.

## 15.8 Managed local Node host — `node/*`

- [ ] **SK-15-043** Spawn/socket/process ownership has the same safety guarantees as Python managed-local mode.
- [ ] **SK-15-044** Unique socket paths prevent cross-client interference.
- [ ] **SK-15-045** Package consumers do not depend on repository-only paths unexpectedly.
- [ ] **SK-15-046** Child exit and startup failure are surfaced deterministically.

## 15.9 TypeScript E2E/package verification

- [ ] **SK-15-047** Typecheck covers the full exported operation typing surface.
- [ ] **SK-15-048** Unit tests cover scalar/value/result/protocol/transport/facade edge cases.
- [ ] **SK-15-049** Real-engine E2E uses the production **standard generated/runtime gRPC** client path; the current hand-written Node HTTP/2 implementation cannot satisfy this item.
- [ ] **SK-15-050** E2E capability coverage is audited against Rust/Python rather than inferred from package existence: two-phase rank-profile execution is currently demonstrated elsewhere but not by the TypeScript ergonomic rank-profile helper.
- [ ] **SK-15-051** Clean npm pack/install/import test succeeds.
- [ ] **SK-15-052** Example compiles and runs against the same public package surface users receive.

---
