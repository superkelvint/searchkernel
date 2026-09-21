# 16. Generation and repository tools — `tools/`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + STATIC`; each tool cited as a gate must itself be executed and its failure behavior tested where a self-test exists.

## 16.1 FlatBuffers/BFBS tools

- [ ] **SK-16-001** `generate-flatbuffers-bfbs.sh` uses pinned `flatc` and canonical schemas.
- [ ] **SK-16-002** `check-flatbuffers-bfbs.sh` detects any schema/BFBS drift.
- [ ] **SK-16-003** `generate-searchkernel-protocol.sh` produces deterministic Rust generated code.
- [ ] **SK-16-004** Python/TypeScript FlatBuffers generation uses the same canonical schema revision.
- [ ] **SK-16-005** Object-API patching for Python is deterministic and tested against regeneration.

## 16.2 gRPC codegen tools

- [ ] **SK-16-006** Rust/Python/TypeScript RPC generation uses standard upstream generators.
- [ ] **SK-16-007** `check-searchkernel-rpc.sh` and `check-python-grpc.sh` detect stale generated outputs.
- [ ] **SK-16-008** TypeScript has an equivalent authoritative generated-stub freshness check.
- [ ] **SK-16-009** No custom Search-Kernel-specific language RPC generator owns standard gRPC framing/stubs.

## 16.3 Native binding/ABI tools

- [ ] **SK-16-010** `check-native-bindings.sh` compares checked-in Rust bindings to fresh bindgen output.
- [ ] **SK-16-011** `check-native-abi.sh` verifies header/library/export/ABI version consistency.
- [ ] **SK-16-012** `check_abi_surface.py` rejects accidental ABI additions/removals/type changes.
- [ ] **SK-16-013** `test-native-bindings.sh` runs C++ contract tests for ownership/lifetimes/noexcept/header isolation.
- [ ] **SK-16-014** ABI tools inspect the same SDK/library actually linked by Rust tests.

## 16.4 Architecture/layout checks

- [ ] **SK-16-015** `check-ir-boundary.sh` enforces dependency firewall and semantic boundaries.
- [ ] **SK-16-016** `check-layout.sh` catches retired/orphaned architecture pieces and invalid repository layout.
- [ ] **SK-16-017** Python FlatBuffers boundary check rejects hand-written parallel full-schema DTO/codecs.
- [ ] **SK-16-018** Capability checker validates declared capabilities against expected status/evidence.

## 16.5 Coverage/hardening tools

- [ ] **SK-16-019** Native adapter coverage measures executable adapter sources, not generated/upstream noise.
- [ ] **SK-16-020** Coverage threshold cannot be satisfied by excluding newly-added modules unintentionally.
- [ ] **SK-16-021** Sanitizer build uses the exact code under test and fails on ASan/UBSan findings.
- [ ] **SK-16-022** Hardening tests run cleanup/error branches via failpoints.

## 16.6 Frontend harness

- [ ] **SK-16-023** Harness state/manifest schemas validate all milestone/reviewer state before execution.
- [ ] **SK-16-024** Verifier integrity checks prevent a verifier from silently mutating its frozen specification/goldens.
- [ ] **SK-16-025** Reviewer/verification status cannot be marked green when required verifier commands were skipped.
- [ ] **SK-16-026** Harness self-tests cover corrupted/missing/stale state.

## 16.7 Retired/orphan tool directories

- [ ] **SK-16-027** Empty/retired `tools/searchkernel-bindgen`, `tools/searchkerneld-e2e-harness`, `core/crates/searchkernel-json`, and `reference/ffi-conformance` directories contain no active code; if retained as empty placeholders, the reason is documented or they are removed.
- [ ] **SK-16-028** No build/test/doc still references obsolete custom bindgen/native-client transport tooling.
- [ ] **SK-16-029** Retired `reference/ffi-conformance` is not an active product acceptance dependency.

---
