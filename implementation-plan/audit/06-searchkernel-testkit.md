# 6. Testkit — `core/crates/searchkernel-testkit`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + UNIT`; additionally prove by `STATIC` that these backends are absent from real-engine acceptance binaries/lanes.

- [ ] **SK-06-001** `RecordingBackend` records the full canonical IR request, not a partial projection.
- [ ] **SK-06-002** `ScriptedBackend` compares complete operation kind + request before returning a scripted response.
- [ ] **SK-06-003** Recording/scripted backends do not implement matching/ranking/filtering/storage behavior.
- [ ] **SK-06-004** Script exhaustion, wrong operation order, and wrong request payload fail loudly.
- [ ] **SK-06-005** Conformance runner uses frozen expected data and never regenerates goldens during verification.
- [ ] **SK-06-006** Conversion tests cover request-side and response-side semantics.
- [ ] **SK-06-007** The testkit can prove client/facade semantics without a Vespa dependency.
- [ ] **SK-06-008** Testkit fixtures cover error paths, not just happy-path requests.

---
