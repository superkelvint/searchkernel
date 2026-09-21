# 10. Raw native bindings — `core/crates/searchkernel-vespa-bindings`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + STATIC + GEN/VER`; ABI compatibility requires header/bindings/export/library evidence from the exact audited native build.

## 10.1 ABI declarations — `src/abi.rs`, `src/bindings.rs`, `src/hardening.rs`

- [ ] **SK-10-001** `src/abi.rs` expected ABI value, `src/bindings.rs`, source header, packaged SDK header, manifest, and shared-library ABI all agree.
- [ ] **SK-10-002** `src/bindings.rs` exactly matches bindgen output from `core/native/vespa/searchkernel_vespa.h`.
- [ ] **SK-10-003** Packaged SDK header exactly matches the source ABI header for the audited build.
- [ ] **SK-10-004** Shared-library export map contains every required public ABI symbol and no unintended symbols.
- [ ] **SK-10-005** ABI version constant matches header, library, manifest, and Rust expectation.
- [ ] **SK-10-006** Integer widths, pointer constness, callback signatures, and ownership contracts match C exactly.
- [ ] **SK-10-007** Struct-of-arrays pointer lifetimes are documented and verified by contract tests.

## 10.2 Build/linking

- [ ] **SK-10-008** SDK discovery cannot accidentally link a different/stale native library.
- [ ] **SK-10-009** RPATH/runtime library lookup matches packaging expectations.
- [ ] **SK-10-010** Build fails closed when source header, SDK header, manifest, or expected ABI version diverge.
- [ ] **SK-10-011** Normal builds do not require libclang/bindgen.
- [ ] **SK-10-012** Explicit binding-regeneration verifier catches drift.

## 10.3 Unsafe/hardening

- [ ] **SK-10-013** Unsafe declarations remain isolated here.
- [ ] **SK-10-014** Test-only hardening/failpoint symbols cannot leak into production linkage unintentionally.
- [ ] **SK-10-015** Native contract tests exercise query lifetime, transport lifetime, schema identity/file descriptors, header namespace, and noexcept behavior.

---
