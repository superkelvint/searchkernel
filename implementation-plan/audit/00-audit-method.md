# Search Kernel Adversarial Correctness Audit Checklist — Evidence-Based Edition

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Purpose:** repository-wide correctness checklist for independent, module-by-module adversarial review of Search Kernel. This document is a set of **proof obligations**, not a list of things that merely look plausible in source.

**Snapshot basis:** uploaded code bundle `searchkern2(20260921-033528)` (archive SHA-256 `bc439271338dc2f13945de9a34fbc9e63930a953556f49c5cda7cea416add3c1`; deterministic clean-tree digest `92b7bb06be749532ca7d30b0ea962e0872e90e38d2243c9f2cf2be4b8f54e437`, computed over sorted relative path + file bytes). The archive bytes are identical to the earlier `searchkern2(20260921-031512)` snapshot despite the different filename.

**Evidence status of this document:** the checklist structure and snapshot-specific findings were derived from the supplied source tree. That does **not** mean the checklist items are already passed. A checkbox may be checked only when an audit record contains the evidence required by this edition.

**Baseline observations made while constructing this edition:** `tools/check-layout.sh` PASS; `tools/check-ir-boundary.sh` PASS; `tools/check-python-flatbuffers-boundary.py` PASS; `tools/check-capabilities.py` FAILS on the pristine export because generated `LIMITATIONS.md` is missing/stale. Cargo/native acceptance was not executed in the construction environment, and the exported native SDK contains no library binary. These observations are starting evidence only; rerun them in the actual audit environment.

## Evidence model

Every checkbox has a stable item ID. An item is not complete merely because a reviewer read the code. The audit record must associate the item ID with one or more evidence records using these classes:

| Code | Evidence class | What counts |
|---|---|---|
| `SRC` | Source evidence | Exact path + symbol/line/range showing the implementation or architectural boundary. |
| `STATIC` | Mechanical static check | Dependency/layout/grep/schema inventory script with command, exit status, and captured output/digest. |
| `GEN` | Generation/freshness evidence | Regeneration from canonical inputs followed by a clean diff/freshness check. |
| `UNIT` | Automated local test | Named unit/integration test exercising the exact behavior. |
| `CONF` | Frozen conformance case | Immutable input + expected output verified without regenerating expected data. |
| `NEG` | Negative-path test | Automated proof that malformed/unsupported/conflicting input fails with the required structured behavior. |
| `ORACLE` | Independent oracle | Direct Vespa C++, pinned Vespa Java behavior, or another explicitly authoritative external reference, compared to Search Kernel. |
| `VER` | Repository verifier | Named verifier script/contract, exact command, exit 0, and its explicit non-goals recorded. |
| `E2E` | Public-path real-engine test | User-facing direct-Rust or first-party client path through the real Vespa engine; mocks/RecordingBackend/ScriptedBackend do not qualify. |
| `PKG` | Clean packaging/bootstrap proof | Build/install/package from a pristine source tree/environment with generated/local residue removed. |
| `HARD` | Hardening evidence | Sanitizer, stress, lifecycle-race, resource-growth, failpoint, or crash/recovery run. |
| `DOC` | Documentation claim | Useful only as a claim to verify. **Never sufficient by itself for PASS.** |

### Minimum proof by claim type

- **Architecture/dependency boundary:** `SRC + STATIC`. If the boundary has a runnable structural test, include it.
- **Pure semantic/parser/conversion behavior:** `SRC + UNIT` or `SRC + CONF`; malformed/unsupported behavior additionally requires `NEG`.
- **Generated contract/freshness:** `SRC + GEN` and, where relevant, a clean `PKG` build.
- **Backend/Vespa behavioral claim:** `SRC + VER`; use `ORACLE` whenever the contract defines one.
- **Public executable capability:** lower-layer evidence **plus `E2E`** for each first-party entry path claimed supported.
- **Unsupported capability:** explicit rejection in source **plus `NEG`** proving it is not silently dropped, approximated, or accepted.
- **Durability/recovery/concurrency/resource claim:** `VER` or `E2E` as appropriate **plus `HARD`** for the claimed failure mode.
- **Cross-language parity:** the same frozen semantic scenario must be executed by every claimed first-party client; source similarity is not evidence.

### Evidence record rules

1. Every checked item must cite at least one evidence ID in the audit ledger, e.g. `EV-0042`.
2. Evidence records contain: item IDs proven, evidence class, exact command or source path/symbol, expected result, observed result, revision/digest, reviewer, date, and verifier non-goals.
3. One evidence record may prove multiple items only when the proof genuinely covers each item; list every item explicitly.
4. Passing a lower layer never implicitly passes an upper layer.
5. If required evidence cannot be run, the item is `UNVERIFIED`, not `PASS`.
6. `N/A` requires evidence that the surface truly does not exist in the audited snapshot.
7. A bug fix is not closed until a regression test exists and the relevant cumulative verifier is rerun.

## Mechanical completeness / closure matrix

The checklist itself is considered complete only if these inventories are regenerated from the audited tree and every discovered entry maps to at least one stable checklist item ID.

| Inventory | Source of truth | Snapshot baseline | Closure requirement |
|---|---|---|---|
| Workspace crates | root `Cargo.toml` workspace members | 11 members | Every crate maps to a numbered module section. |
| Native adapter surface | `core/native/vespa/*.{cc,h}` + maps/build scripts | concrete source/header/helper set in §12 | Every production/native ABI/resource file maps to §10–§12. |
| First-party clients | package roots registered in `reference/client-contracts/catalog.json` and tracked files under `clients/` | Current registered production clients: Python, TypeScript, PHP, and Ruby | `python3 tools/check-audit-client-coverage.py` regenerates the file inventory; every first-party client maps to a dedicated module, selected transport contract, evidence roots, and existing parity/E2E gates. |
| Canonical domain schemas | `api/*.fbs` | `searchkernel.fbs`, request envelope, response envelope | Every table/union/enum/field family is represented in schema/IR/round-trip coverage. |
| RPC contract | `api/rpc/searchkernel.proto` | one transport schema | Every protobuf message/service is transport-only and generated through standard tooling. |
| Portable operations | request/response union variants | derive from current schemas | Every operation has ingress, IR, dispatch, response, error, and negative-path coverage. |
| Production generated-source dependencies | `include!(...)` / generated imports | Rust protocol, Rust RPC, TS generated trees are absent in this export | Every dependency has a deterministic clean bootstrap/freshness proof. |
| Verifier lanes | executable scripts/contracts under `reference/` | M0–M13 + specialized/I/Phase lanes present | Every active verifier has an exact “proves / does not prove” mapping in §17. |
| Public first-party facade families | exported public source/API | derive mechanically/manual API inventory | Every public operation/builder family maps to semantic + E2E evidence or explicit unsupported status. |
| Capability/documentation claims | `docs/capabilities.json`, generated docs, README/status | known stale/incomplete claims exist | Every advertised claim points to executable evidence; docs never create support by themselves. |

**Freeze condition:** after the closure matrix is regenerated and shows zero unmapped implementation/API/verifier surfaces, this checklist is frozen for the snapshot. Future additions require a concrete unmapped artifact, newly added scope, or demonstrated checklist defect; “I thought of another edge case” is not sufficient change control.

## Revision log

| Edition | Basis | Change |
|---|---|---|
| Evidence-based | `033528` / digest above | Added stable item IDs, evidence classes, section proof minima, mechanical closure matrix, evidence ledger, known-finding closure semantics, and checklist freeze/change control. |
| Prior v2 | same archive bytes | Re-vetted paths/verifier scope and corrected several hallucinated/overstated proof claims. |

## Codebase-vetted repository inventory

The audit must begin from the modules that actually exist in this snapshot. Do not infer modules from roadmap prose.

- [ ] **SK-META-001** Workspace crates are inventoried from `Cargo.toml`: `flatbuffers-build`, `searchkernel-protocol`, `searchkernel-filter`, `searchkernel-ir`, `searchkernel`, `searchkernel-testkit`, `searchkernel-dispatch`, `searchkernel-ir-adapter`, `searchkernel-vespa-bindings`, `searchkernel-vespa-engine`, and `searchkerneld`.
- [ ] **SK-META-002** Native implementation inventory includes `core/native/vespa/{abi,engine,document,update,query,search,grouping,tensor,errors,hardening}.cc`, `searchkernel_vespa.h`, `searchkernel_vespa_internal.h`, test-only ABI headers/maps, and resource-owner helpers.
- [ ] **SK-META-003** First-party non-Rust clients are inventoried from package roots registered in `reference/client-contracts/catalog.json` and actual tracked files under `clients/`; the current inventory includes Python, TypeScript, PHP, and Ruby, and must be regenerated for each audited revision.
- [ ] **SK-META-004** Active native verifier baseline is read from `reference/vespa/README.md`; for this snapshot it is M13, with I-series/Phase 10 plus specialized Phase 9, SearchReply, recovery, recursive-grouping, and grouping/YQL lanes.
- [ ] **SK-META-005** Empty historical/placeholder directories (`tools/searchkernel-bindgen`, `tools/searchkerneld-e2e-harness`, `core/crates/searchkernel-json`, `reference/ffi-conformance`, and `reference/json-conformance`) are treated as archaeology/placeholders only unless a build/test/document still references them. An empty directory is never counted as conformance evidence.
- [ ] **SK-META-006** Generated source dependencies are inventoried separately from checked-in source. In this snapshot `core/crates/searchkernel-protocol/src/generated`, `searchkerneld/src/generated`, and `clients/typescript/src/generated` are absent even though production source imports/includes them.
- [ ] **SK-META-007** Native SDK packaging state is inventoried separately from source headers/manifests. In this exported snapshot `core/native/sdk/linux-x86_64/lib/` contains no library file, so the export by itself cannot prove consumer link/run behavior.
- [ ] **SK-META-008** Generated/build residue in the supplied tree (`dist`, `.example-dist`, `.pytest_cache`, `*.egg-info`) is classified as intentional artifact, packaging input, or repository hygiene defect; it must never be mistaken for source-of-truth evidence.


## How to use this checklist

- [ ] **SK-META-009** Record the exact Git commit/revision being audited. If Git metadata is unavailable (for example, an exported source bundle), record the archive SHA-256 and a deterministic source-tree digest.
- [ ] **SK-META-010** Enumerate the actual workspace/client/native/verifier modules before reviewing checklist sections. Any checklist section with no corresponding implementation must be marked `N/A`, not silently passed. Any implementation module with no checklist coverage is a checklist defect.
- [ ] **SK-META-011** Read `reference/vespa/README.md` before using milestone contracts. Historical M-series contracts are frozen compatibility evidence, not necessarily a description of the latest portable/product surface.
- [ ] **SK-META-012** Treat `docs/capabilities.json`, generated `CAPABILITIES.md`, README prose, examples, and roadmap text as claims to verify, not implementation authority.
- [ ] **SK-META-013** Start from a clean checkout/worktree; no generated files, native SDKs, sockets, caches, or persisted indexes may be inherited from an earlier run unless the test explicitly requires them.
- [ ] **SK-META-014** Do not mark an item complete from code inspection alone when the behavior can be exercised by a test or verifier.
- [ ] **SK-META-015** For every bug found, add a regression test that fails before the fix and passes after the fix.
- [ ] **SK-META-016** Mark an item complete only when the implementation is correct **and** appropriate automated regression/unit/integration coverage exists.
- [ ] **SK-META-017** Record the evidence for each completed module: commands run, relevant tests/verifiers, and any oracle/golden used.
- [ ] **SK-META-018** Treat skipped/ignored tests as unverified unless the skip is itself part of the contract.
- [ ] **SK-META-019** Treat unsupported capability as correct only when it is rejected explicitly and deterministically; silent degradation is a failure.
- [ ] **SK-META-020** Do not regenerate frozen expected/oracle data while validating an implementation.
- [ ] **SK-META-021** Re-run the relevant cumulative verifier after any fix that crosses module boundaries.

## Evidence discipline — prevent false PASS

- [ ] **SK-META-022** Maintain a machine-derived inventory of production source files and map every file/module to at least one checklist section; an unmapped production module is a checklist defect.
- [ ] **SK-META-023** Record **what layer each piece of evidence proves**: parser syntax, IR semantics, portable encoding, adapter lowering, native ABI, real-engine behavior, public Rust API, daemon transport, language facade, or cross-language parity.
- [ ] **SK-META-024** Do not use a lower-layer verifier to claim a higher-layer behavior. A native ABI grouping verifier does not prove portable result encoding; a public Rust E2E does not prove Python/TypeScript parity; an IR golden does not prove real Vespa execution.
- [ ] **SK-META-025** When a verifier has explicit non-goals, copy those non-goals into the audit record. Absence from a verifier is `UNVERIFIED`, not implicit success.
- [ ] **SK-META-026** Every claim in `docs/capabilities.json` / generated docs must identify an evidence path that actually exercises the claimed layer. Merely mentioning a source file or example is not a verifier.
- [ ] **SK-META-027** Every checklist assertion about a verifier is checked against the verifier script **and** its contract/README before being used as evidence.
- [ ] **SK-META-028** Use `PASS`, `FAIL`, `UNVERIFIED`, and `N/A` distinctly. `N/A` requires a documented reason; `UNVERIFIED` means the implementation may exist but the required proof has not been run or does not exist.

---

# 0. Repository-wide architectural invariants

**Minimum evidence to PASS this section:** `SRC + STATIC`; runtime claims inside this section additionally require the relevant lower-layer `UNIT/VER/E2E/NEG` evidence.

- [ ] **SK-00-001** FlatBuffers under `api/` are the sole canonical portable domain contract.
- [ ] **SK-00-002** Protobuf contains transport framing only and does not duplicate Search Kernel domain operations or fields.
- [ ] **SK-00-003** `searchkernel-ir` is the sole portable semantic boundary for defaults, normalization, validation, and explicit request meaning.
- [ ] **SK-00-004** Direct Rust requests and portable requests converge on the same semantic resolver operations.
- [ ] **SK-00-005** No transport implements Search Kernel defaults, field legality, ranking semantics, filter semantics, grouping semantics, or result semantics.
- [ ] **SK-00-006** No client independently parses the Search Kernel textual filter language.
- [ ] **SK-00-007** No client that selects gRPC/gRPC-Web implements its own protocol/framing stack; every such client uses standard generated gRPC/gRPC-Web tooling appropriate to that runtime.
- [ ] **SK-00-008** `searchkernel-ir-adapter` is the only production crate allowed to **directly import both** `searchkernel-ir` and `searchkernel-vespa-engine`. The product `searchkernel` crate may reach the adapter behind its optional `native` feature; that does not make it a second lowering implementation.
- [ ] **SK-00-009** `searchkernel-ir`, `searchkernel-protocol`, `searchkernel-filter`, `searchkernel-testkit`, and dispatcher-only semantic tests compile without Vespa implementation crates/native SDK requirements.
- [ ] **SK-00-010** Vespa engine crates do not depend on portable IR semantics except through the adapter boundary.
- [ ] **SK-00-011** Vespa C++ types never escape the native adapter into public Rust APIs.
- [ ] **SK-00-012** Raw FFI/native calls are confined to the bindings/native boundary.
- [ ] **SK-00-013** Recording/scripted backends never participate in real-engine acceptance.
- [ ] **SK-00-014** Unsupported backend capabilities return structured `Unsupported` errors instead of being ignored, approximated, or reinterpreted.
- [ ] **SK-00-015** No retired portable client FFI/native transport path has re-entered the product architecture; empty historical directories and history docs do not count as active paths, but any live dependency/script/package reference does.
- [ ] **SK-00-016** Source-distribution bootstrap is closed: every `include!("generated/...")` or client import from a generated tree is satisfied either by checked-in generated files or by an automatic documented build/generation step that runs before normal build/test/package commands.
- [ ] **SK-00-017** Rust remains directly embedded and does not serialize through FlatBuffers/gRPC to reach the local engine.
- [ ] **SK-00-018** Every first-party non-Rust product client present in the tree uses one explicitly sanctioned, mechanically schema-derived transport path that converges on canonical FlatBuffer envelope bytes and the shared dispatcher before Search Kernel semantics are applied. Binary gRPC/gRPC-Web clients use standard generated RPC tooling; explicitly designated HTTP-first clients use standard FlatBuffers JSON through the official BFBS-backed parser/printer. A custom REST/domain DTO model, handwritten JSON -> IR path, or handwritten gRPC/HTTP2 framing is a failure of this invariant.
- [ ] **SK-00-019** HTTP-first status is explicit per client and is not a general exemption from transport discipline. Such clients must prove representative request/response/value/union fidelity against the canonical BFBS bridge and must not own Search Kernel defaults, parsing, validation, normalization, or result semantics.

---
