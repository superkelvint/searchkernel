# 1. Portable API contract — `api/`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + GEN + CONF`; presence/invalid/unknown variants require `NEG`. No engine evidence is required merely to prove representability.

## 1.1 `api/searchkernel.fbs`

- [ ] **SK-01-001** Every public portable value is representable losslessly.
- [ ] **SK-01-002** Scalar signed/unsigned 64-bit integer policy is consistent across schema, JSON, Rust, Python, and TypeScript.
- [ ] **SK-01-003** Non-finite floating-point values are represented intentionally and round-trip losslessly where allowed.
- [ ] **SK-01-004** Tensor value unions distinguish dense, sparse, and mixed representations without ambiguity.
- [ ] **SK-01-005** Optional/presence-sensitive fields preserve absent vs explicitly empty/zero values where semantics differ.
- [ ] **SK-01-006** Union discriminators are stable and verified against generated artifacts/goldens.
- [ ] **SK-01-007** Query expression variants cover the currently advertised portable query surface.
- [ ] **SK-01-008** Filter representation preserves first-class `IN`, boolean structure, range bounds, and clause-local textual filter bindings.
- [ ] **SK-01-009** Grouping/result schemas preserve heterogeneous recursive `ResultNode` trees, including hits nested inside groups.
- [ ] **SK-01-010** Ranking profiles, query inputs, rank properties, query properties, and ranking request state remain distinct namespaces.
- [ ] **SK-01-011** Schema definitions preserve rank profiles, document summaries, field capabilities, analyzers, tensors, and nested structures without lossy encode/decode.
- [ ] **SK-01-012** `IndexDefinition` rich-schema families are inventoried explicitly rather than hidden behind "schema fidelity": `document_type`, schema/document inheritance, structs, fieldsets, search-only schema fields, imported fields, document summaries, rank profiles, constants, and ONNX models.
- [ ] **SK-01-013** Data-type coverage explicitly includes scalar, array/map/weighted-set, struct/reference, predicate, position, tensor, and any presence/config metadata represented by the schema; unsupported execution does not excuse lossy IR round trips.
- [ ] **SK-01-014** `QueryModel` / `QueryModelType` fields (`query_string`, `default_index`, `encoding`, `filter`, `language`, `locale`, composite/tokenization/syntax/profile/YQL-default type controls, `sources`, `restrict`, `search_path`, `search_group`) remain distinct, lossless caller intent.
- [ ] **SK-01-015** `PresentationRequest` fields (`summary`, `fields`, `highlighting`, `format`, `bolding`, `timing`) preserve presence and are not conflated with result decoding.
- [ ] **SK-01-016** `query_profile` and `search_chain` are represented distinctly from generic query properties and from ranking profile selection.
- [ ] **SK-01-017** Mutation conditions and update-operation variants preserve all represented caller intent.
- [ ] **SK-01-018** Error structures retain machine-readable code, path/details, retryability/transience, and metadata.

## 1.2 `api/searchkernel_request.fbs`

- [ ] **SK-01-019** Request envelope versioning rejects stale/future/unknown protocol versions deterministically.
- [ ] **SK-01-020** Every supported operation has exactly one unambiguous request payload discriminator.
- [ ] **SK-01-021** Unknown/missing payloads are rejected before semantic dispatch.
- [ ] **SK-01-022** Request envelopes cannot be confused with response envelopes or unrelated FlatBuffer roots.

## 1.3 `api/searchkernel_response.fbs`

- [ ] **SK-01-023** Response envelope correctly discriminates successful payloads and portable errors.
- [ ] **SK-01-024** Every request operation has the expected response shape.
- [ ] **SK-01-025** Result-tree, coverage, timing, trace, metadata, errors, and feature values are representable without client-specific assumptions.
- [ ] **SK-01-026** Response encoding preserves infinite relevance and other non-JSON-native values explicitly.

## 1.4 `api/rpc/searchkernel.proto`

- [ ] **SK-01-027** RPC remains a bytes-only frame service (`Execute(Frame) -> Frame`).
- [ ] **SK-01-028** No Search Kernel operation/domain field has leaked into protobuf.
- [ ] **SK-01-029** Generated server and client stubs are produced by standard tooling.
- [ ] **SK-01-030** RPC schema changes are backward-compatible or explicitly versioned.

## 1.5 Contract generation

- [ ] **SK-01-031** `.fbs -> BFBS` generation is reproducible with the pinned FlatBuffers version.
- [ ] **SK-01-032** `.fbs -> Rust/Python/TypeScript generated models` is reproducible.
- [ ] **SK-01-033** `.proto -> Rust/Python/TypeScript generated stubs` is reproducible.
- [ ] **SK-01-034** Checked-in generated artifacts are byte-for-byte/currently equivalent to fresh generation where they are intentionally checked in.
- [ ] **SK-01-035** No generated file is hand-edited as the source of truth.

---
