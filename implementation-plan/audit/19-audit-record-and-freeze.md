# 25. Audit record template

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** This section defines the mandatory evidence ledger format; use it for every checked item.

Use one copy per module/review pass.

```text
Audit record ID: AR-____
Module / section:
Audited revision or archive/tree digest:
Reviewer:
Date:

Checklist item IDs covered:

Evidence records:
- EV-____  class=SRC|STATIC|GEN|UNIT|CONF|NEG|ORACLE|VER|E2E|PKG|HARD
  items: SK-...
  source path/symbol OR exact command:
  expected result:
  observed result:
  output/log/artifact digest or path:
  verifier contract/non-goals:

Source files reviewed:

Commands/tests/verifiers run:

Oracle/golden provenance:

Findings:
- BLOCKER:
- HIGH:
- MEDIUM:
- LOW:

Regression tests added/confirmed:

Unverified / N/A item IDs and reasons:

Final status: PASS / FAIL / PARTIAL / UNVERIFIED
```

---

# 26. Checklist self-audit before sign-off

**Minimum evidence to PASS this section:** `STATIC + GEN/PKG/VER` as applicable. Sign-off is prohibited while any closure-matrix entry is unmapped.

- [ ] **SK-26-001** Re-run a source-tree inventory and confirm every production crate/client/native module added since this checklist was written maps to a section above.
- [ ] **SK-26-002** Re-run the verifier inventory from scripts on disk; remove or downgrade any checklist claim that exceeds a verifier's actual commands/contract.
- [ ] **SK-26-003** Re-run generated-directory/package inventory from a pristine checkout/export; local generated/cache files must not hide bootstrap defects.
- [ ] **SK-26-004** Re-run capability-ledger reconciliation against tests and executable gates; examples/source presence alone are not sufficient evidence.
- [ ] **SK-26-005** Search for `TODO`, `FIXME`, `unimplemented!`, `todo!`, panic placeholders, ignored tests, expected failures, `Unsupported`, and feature gates on public request paths; classify each against advertised capability.
- [ ] **SK-26-006** Compare canonical schema unions/tables against `*_to_ir` match arms and IR -> API encoders mechanically where practical; every represented variant is handled, explicitly unsupported at the correct stage, or covered by a negative test.
- [ ] **SK-26-007** Compare first-party client ergonomic surfaces against the capability ledger by language; absence of sugar is documented independently from represented/semantic/executable support.
- [ ] **SK-26-008** A reviewer who did not implement the fixes reruns the final gates from clean state and signs the audit record.

---

# 27. Checklist freeze and change control

**Minimum evidence to PASS this section:** `STATIC` inventory output proving zero unmapped surfaces, plus reviewer sign-off.

- [ ] **SK-27-001** The closure matrix has been regenerated against the exact audited revision and contains zero unmapped production modules, canonical schema families, public first-party facade families, generated-source dependencies, and active verifier lanes.
- [ ] **SK-27-002** Every checklist item has a stable ID and every checked item has at least one evidence record satisfying its section's minimum proof requirement.
- [ ] **SK-27-003** Every active verifier is mapped to explicit claims it proves and explicit adjacent claims it does **not** prove.
- [ ] **SK-27-004** Every advertised capability is linked to executable evidence at the correct layer; documentation/source presence alone is not accepted.
- [ ] **SK-27-005** This checklist is frozen for the audited snapshot after the previous items pass.
- [ ] **SK-27-006** A future checklist change is accepted only when classified as exactly one of: `CHECKLIST_DEFECT` (an existing concrete surface was unmapped), `CHECKLIST_CORRECTION` (an item misstated actual code/verifier evidence), `CODE_SCOPE_CHANGE` (the repository added/removed a concrete surface), or `NEW_BUG_CLASS` (a demonstrated defect is not expressible by an existing proof obligation).
- [ ] **SK-27-007** Every future checklist change names the concrete triggering artifact (path/schema symbol/public API/verifier/failing regression), affected item IDs, and why an existing item could not capture it.
- [ ] **SK-27-008** Purely speculative edge cases that are already subsumed by an existing correctness property become test cases/evidence under that item rather than new checklist items.
- [ ] **SK-27-009** Checklist revisions maintain an explicit change log so reviewers can distinguish newly discovered code defects from changes to the audit specification itself.
