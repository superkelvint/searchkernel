# 8. Canonical dispatcher — `core/crates/searchkernel-dispatch`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT/CONF + NEG`; direct-vs-gRPC semantic identity requires a separate parity test and is not implied by dispatcher unit tests.

- [ ] **SK-08-001** Arbitrary bytes are verified as the correct FlatBuffer root before field access.
- [ ] **SK-08-002** Request envelope protocol version is checked before dispatch.
- [ ] **SK-08-003** Unknown/missing operation discriminators fail deterministically.
- [ ] **SK-08-004** Every supported operation routes through its canonical `*_to_ir()` conversion.
- [ ] **SK-08-005** No operation builds backend IR manually inside the dispatcher.
- [ ] **SK-08-006** Malformed bytes cannot panic or trigger undefined behavior.
- [ ] **SK-08-007** Closed-target/session errors map to stable portable errors.
- [ ] **SK-08-008** Semantic errors become canonical response-envelope errors.
- [ ] **SK-08-009** Backend results/errors are encoded using shared IR -> API conversion.
- [ ] **SK-08-010** Dispatcher has no gRPC/HTTP/socket-specific logic.
- [ ] **SK-08-011** Direct portable dispatch and gRPC-carried dispatch produce identical IR.
- [ ] **SK-08-012** Size/complexity abuse is bounded at an appropriate outer layer before excessive allocation.

---
