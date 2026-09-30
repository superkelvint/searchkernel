# Search Kernel Adversarial Audit Checklist — Modular Edition

This modular checklist began as a **lossless split** of `searchkernel-adversarial-audit-checklist-evidence-based.md` into reviewable module files. The original IDs remain stable evidence keys; the modular checklist is now also extended under the change-control rules when the repository adds concrete new attack/architecture surfaces.

**Snapshot basis:** `searchkern2(20260921-033528)`  
**Snapshot archive SHA-256:** `bc439271338dc2f13945de9a34fbc9e63930a953556f49c5cda7cea416add3c1`  
**Source checklist SHA-256:** `164b85f4687d25ce2449cac88644d032cdf6fdcf60c2c7a3b1b5f5e1a3a29848`  
**Checklist obligations:** 990 unique stable item IDs.

## How to use the split

Start with [00-audit-method.md](00-audit-method.md). It contains the evidence taxonomy, proof minima, mechanical closure matrix, repository inventory, and repository-wide architectural invariants. Then assign individual module files to independent reviewers. Cross-module claims belong in [18-cross-cutting-gates.md](18-cross-cutting-gates.md), and all evidence is recorded using [19-audit-record-and-freeze.md](19-audit-record-and-freeze.md).

A module reviewer should not silently pass an item based on another module's evidence. Record the evidence ID and proof class required by the audit method. If a finding crosses module boundaries, link the same finding/evidence record to every affected stable item rather than duplicating the obligation.

## Module index

| Review area | File | Scope |
|---|---|---|
| Audit method & architecture | [00-audit-method.md](00-audit-method.md) | Evidence model, closure matrix, repository inventory, usage rules, repository-wide invariants |
| Portable API | [01-api-portable-contract.md](01-api-portable-contract.md) | `api/*.fbs`, request/response envelopes, RPC schema, generation |
| FlatBuffers build helper | [02-flatbuffers-build.md](02-flatbuffers-build.md) | `core/crates/flatbuffers-build` |
| Protocol | [03-searchkernel-protocol.md](03-searchkernel-protocol.md) | `core/crates/searchkernel-protocol` |
| Filter parser | [04-searchkernel-filter.md](04-searchkernel-filter.md) | `core/crates/searchkernel-filter` |
| Semantic IR | [05-searchkernel-ir.md](05-searchkernel-ir.md) | `core/crates/searchkernel-ir` |
| Testkit | [06-searchkernel-testkit.md](06-searchkernel-testkit.md) | `core/crates/searchkernel-testkit` |
| Rust product API | [07-searchkernel-rust-api.md](07-searchkernel-rust-api.md) | `core/crates/searchkernel` |
| Dispatcher | [08-searchkernel-dispatch.md](08-searchkernel-dispatch.md) | `core/crates/searchkernel-dispatch` |
| IR → Vespa adapter | [09-searchkernel-ir-adapter.md](09-searchkernel-ir-adapter.md) | `core/crates/searchkernel-ir-adapter` |
| Raw native bindings | [10-searchkernel-vespa-bindings.md](10-searchkernel-vespa-bindings.md) | `core/crates/searchkernel-vespa-bindings` |
| Safe Vespa engine | [11-searchkernel-vespa-engine.md](11-searchkernel-vespa-engine.md) | `core/crates/searchkernel-vespa-engine` |
| Native C++ adapter | [12-native-vespa-cpp.md](12-native-vespa-cpp.md) | `core/native/vespa` |
| Daemon / RPC host | [13-searchkerneld.md](13-searchkerneld.md) | `searchkerneld` |
| Python client | [14-client-python.md](14-client-python.md) | `clients/python` |
| TypeScript client | [15-client-typescript.md](15-client-typescript.md) | `clients/typescript` |
| PHP client | [28-client-php.md](28-client-php.md) | `clients/php` |
| Ruby client | [29-client-ruby.md](29-client-ruby.md) | `clients/ruby` |
| Generation/repository tools | [16-tools.md](16-tools.md) | `tools/` |
| Verifier lanes | [17-reference-verifiers.md](17-reference-verifiers.md) | `reference/` conformance/oracle/verifier surfaces |
| Cross-cutting gates | [18-cross-cutting-gates.md](18-cross-cutting-gates.md) | Capabilities/docs, packaging, parity, E2E, bypass search, final gates, snapshot closure |
| Audit record & freeze | [19-audit-record-and-freeze.md](19-audit-record-and-freeze.md) | Evidence ledger template, checklist self-audit, freeze/change control |

## Split integrity

The modular set is mechanically verified so that every stable checklist ID appears **exactly once** across the module files. The original 908-ID source set is frozen by count and ID-set digest; live scope extensions are recorded in `split-manifest.json` and must account for every later ID. The index intentionally contains no duplicated checklist obligations.

Run `python3 tools/check-audit-split.py` to verify item uniqueness, per-file counts and hashes, total counts, and live-extension metadata. Run `python3 tools/check-audit-client-coverage.py` to regenerate the first-party client file inventory from `reference/client-contracts/catalog.json` and verify every discovered client file maps to a dedicated audit module.

## Live checklist extensions

- **2026-09-24 — `CODE_SCOPE_CHANGE`: first-party transport contract reconciliation.** Issue/PR #596 added `SK-00-019` and `SK-16-030`; this historical extension is explicitly represented so the frozen 908-ID source set remains mechanically distinguishable from later obligations.
- **2026-09-23 — `CODE_SCOPE_CHANGE`: public Search Endpoints.** Issue #469 / PR #470 adds a new anonymous/public-serving ingress surface inside `searchkerneld`. `13-searchkerneld.md` now includes `SK-13-029..SK-13-110` covering public capability isolation, raw/decoded bounds, aggregate-memory and response-amplification limits, computational DoS, parser differential/injection defenses, connection/admission abuse, fuzz/property/stress requirements, privacy/error/logging boundaries, release gates, and implementation-specific closure checks derived from PR #470.
- **2026-09-30 — `CODE_SCOPE_CHANGE`: first-party PHP and Ruby clients.** Issue #935 adds dedicated `28-client-php.md` and `29-client-ruby.md` modules for current production client roots plus `SK-16-031..SK-16-032` for the inventory and split-integrity checks. The change maps the existing `SK-20-*`, `SK-21-013`, `SK-22-002/004`, `SK-23-016/019`, `SK-26-001/004/007`, and `SK-27-001/004/006/007/009` gates to both modules without duplicating those cross-cutting obligations.
