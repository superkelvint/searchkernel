# 3. Protocol crate — `core/crates/searchkernel-protocol`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + GEN`; malformed/root/version/union failures require `NEG`; JSON semantic equivalence requires dispatcher/IR evidence, not printer output alone.

## 3.1 Generated protocol module — `src/lib.rs`, `src/protocol.rs`, generated tree

- [ ] **SK-03-001** `src/lib.rs` and `src/protocol.rs` include only files generated from the current V2 schemas; missing generated include targets are a clean-checkout/build failure, not an environment excuse.
- [ ] **SK-03-002** Generated Rust types exactly match current `.fbs` schemas.
- [ ] **SK-03-003** Root identifiers/version assumptions are correct for request/response verification.
- [ ] **SK-03-004** No semantic defaults or validation live in generated/wire-facing code.
- [ ] **SK-03-005** Borrowed FlatBuffer views do not escape their buffer lifetime.

## 3.2 Standard FlatBuffers JSON — `src/json.rs`, `build.rs`, `native/flatbuffers_json_bridge.{cc,h}`

- [ ] **SK-03-006** JSON parsing uses the official schema-aware FlatBuffers parser, not a hand-written Search Kernel dialect.
- [ ] **SK-03-007** JSON printing uses the official schema-aware FlatBuffers printer.
- [ ] **SK-03-008** JSON -> FlatBuffer -> JSON preserves supported values and union structure.
- [ ] **SK-03-009** Int64/UInt64 decimal-string policy is preserved.
- [ ] **SK-03-010** NaN/Infinity handling follows the canonical contract and rejects illegal representations.
- [ ] **SK-03-011** Unknown fields, malformed unions, wrong root types, and invalid enum values fail closed.
- [ ] **SK-03-012** BFBS used by JSON is generated from the same canonical `.fbs` revision as Rust bindings.
- [ ] **SK-03-013** JSON conversion never bypasses `*_to_ir()` semantics after producing FlatBuffer bytes.
- [ ] **SK-03-014** The C++ JSON bridge is memory-safe across FFI: output buffers/error buffers have explicit ownership, are freed exactly once, and parser/printer exceptions cannot cross the ABI.
- [ ] **SK-03-015** `build.rs` verifies the exact FlatBuffers C++ header/library version and cannot accidentally link a second system FlatBuffers implementation.

## 3.3 Envelope/root invariants

- [ ] **SK-03-016** Request and response identifiers remain exactly `SKRQ` and `SKRS`; dispatcher/client tests reject swapped roots even if the underlying table shape happens to verify.
- [ ] **SK-03-017** V2 `protocol_version` is physically present in canonical requests/responses; legacy envelopes whose version field is omitted decode as zero and are rejected.
- [ ] **SK-03-018** Request/response union tags and payload tables agree; a tag/payload mismatch is rejected before semantic conversion.
- [ ] **SK-03-019** Duplicate semantic map keys represented as FlatBuffers vectors are rejected centrally rather than accepted with first/last-wins behavior.

---
