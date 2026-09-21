# 2. FlatBuffers build helper — `core/crates/flatbuffers-build`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT + PKG`; reproducibility/cache claims require a clean rebuild from empty generated/build state.

- [ ] **SK-02-001** The helper exposes only build/link metadata required by `searchkernel-protocol`.
- [ ] **SK-02-002** Native FlatBuffers version is exactly the repository-pinned version.
- [ ] **SK-02-003** Compiler/toolchain fingerprint invalidates stale incompatible native build output.
- [ ] **SK-02-004** A failed link/probe cannot leave a partially valid cache treated as usable.
- [ ] **SK-02-005** Clean rebuild logic removes only the intended FlatBuffers output, not unrelated workspace artifacts.
- [ ] **SK-02-006** Build behavior is deterministic across clean runs on the supported platform.

---
