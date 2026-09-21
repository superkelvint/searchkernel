# 12. Native C++ adapter — `core/native/vespa`

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + VER`; behavioral compatibility requires `ORACLE` where milestone contracts define one; safety/lifecycle claims require sanitizer/failpoint evidence.

## 12.1 Public ABI — `searchkernel_vespa.h`, `abi.cc`, export maps, internal/test headers

- [ ] **SK-12-001** Header declarations, definitions, version script/export map, SDK header, and Rust bindings are synchronized atomically.
- [ ] **SK-12-002** Every exported function catches C++ exceptions and returns ABI-safe errors.
- [ ] **SK-12-003** No C++ exception crosses the C ABI.
- [ ] **SK-12-004** Opaque handles are type-safe enough to reject wrong-handle misuse where practical.
- [ ] **SK-12-005** ABI additions preserve backward compatibility or intentionally bump the ABI contract.
- [ ] **SK-12-006** libabigail/release-baseline checks cover data/function ABI changes.
- [ ] **SK-12-007** `searchkernel_vespa_internal.h` is never installed/exposed as public SDK API and does not create a second ABI contract accidentally consumed by Rust.
- [ ] **SK-12-008** `searchkernel_vespa_test.h` and test export map symbols are reachable only in explicit hardening/native-contract builds, never production packages.

## 12.2 Engine — `engine.cc`

- [ ] **SK-12-009** Proton/Vespa startup/shutdown ordering is valid.
- [ ] **SK-12-010** Engine open validates configuration/persistent state before accepting requests.
- [ ] **SK-12-011** Reopen after graceful shutdown preserves data.
- [ ] **SK-12-012** Initialization failure unwinds all partially-created resources.
- [ ] **SK-12-013** Concurrency primitives and ownership match Vespa expectations.

## 12.3 Documents — `document.cc`

- [ ] **SK-12-014** All supported scalar/tensor/document fields map to Vespa document types correctly.
- [ ] **SK-12-015** Field lookup failures are explicit.
- [ ] **SK-12-016** String/byte/tensor lifetimes remain valid during native ingestion.
- [ ] **SK-12-017** GET/docsum values are copied into Search Kernel-owned ABI values safely.

## 12.4 Updates — `update.cc`

- [ ] **SK-12-018** Assign/clear/arithmetic update semantics use Vespa's native DocumentUpdate path.
- [ ] **SK-12-019** Ordered update operations preserve required ordering.
- [ ] **SK-12-020** Unsupported mutations are rejected before partial application.
- [ ] **SK-12-021** Update failures do not leave partially-applied Search Kernel-side state.

## 12.5 Query — `query.cc`

- [ ] **SK-12-022** Query-tree construction uses native Vespa items and correct field metadata.
- [ ] **SK-12-023** Borrowed strings/arrays remain valid for Vespa's required lifetime.
- [ ] **SK-12-024** Term weights/annotations/filter/ranked flags map correctly.
- [ ] **SK-12-025** IN uses native membership where expected.
- [ ] **SK-12-026** NN exact/approximate options map to the correct native query item/HNSW path.
- [ ] **SK-12-027** SameElement relative member handling is correct.
- [ ] **SK-12-028** Predicate and geo encoding match direct Vespa oracle behavior.

## 12.6 Search — `search.cc`

- [ ] **SK-12-029** Matcher/ranking execution is Vespa-native; no duplicate scoring logic exists.
- [ ] **SK-12-030** Rank-profile selection and query/rank properties reach the correct Vespa request context.
- [ ] **SK-12-031** First/second-phase ranking runs as configured.
- [ ] **SK-12-032** Match/summary feature extraction is lifetime-safe and type-correct.
- [ ] **SK-12-033** Total hits, coverage, degradation and result hits are read from authoritative native structures.
- [ ] **SK-12-034** Docsums/summary-class fill path is local and does not assume distributed fill services.

## 12.7 Grouping — `grouping.cc`

- [ ] **SK-12-035** Grouping expressions/operators map exactly to supported Vespa grouping primitives.
- [ ] **SK-12-036** Count/sum/avg/min/max/stddev/xor/quantiles return correct value types.
- [ ] **SK-12-037** Nested groups preserve hierarchy/order.
- [ ] **SK-12-038** Summary aggregation emits hit IDs, relevance/score, and scalar summary fields correctly.
- [ ] **SK-12-039** Group-level max/precision/order semantics match Vespa oracle.
- [ ] **SK-12-040** Unsupported arbitrary tree structures fail instead of being flattened.

## 12.8 Tensor — `tensor.cc`

- [ ] **SK-12-041** Tensor type specifications are parsed/validated correctly.
- [ ] **SK-12-042** Dense tensor cell data is copied/owned safely.
- [ ] **SK-12-043** Query/document tensor paths use compatible Vespa tensor representations.
- [ ] **SK-12-044** Invalid dimensions/cell counts are rejected without memory corruption.

## 12.9 Errors/hardening — `errors.cc`, `hardening.cc`

- [ ] **SK-12-045** Thread-local/global error storage has safe lifetime and concurrency behavior.
- [ ] **SK-12-046** Error messages/codes survive long enough for Rust to copy them.
- [ ] **SK-12-047** Test failpoints cannot affect production unless explicitly enabled.
- [ ] **SK-12-048** Failure injection validates cleanup of handles, transport objects, file descriptors, and temporary resources.

## 12.10 Resource helpers — `file_descriptor.h`, `transport_owner.h`

- [ ] **SK-12-049** Move/copy semantics prevent double-close/double-destroy.
- [ ] **SK-12-050** File descriptors are closed on all error paths.
- [ ] **SK-12-051** Transport ownership/destruction order is safe during startup/shutdown failure.

## 12.11 Native SDK packaging — `build-sdk.sh`, `manifest.json`, `core/native/sdk/*`

- [ ] **SK-12-052** SDK is built from the exact pinned Vespa/Search Kernel source revision.
- [ ] **SK-12-053** SDK contains required headers/libraries/runtime dependencies for normal Rust builds.
- [ ] **SK-12-054** SDK manifest records ABI/version/toolchain/source identity accurately.
- [ ] **SK-12-055** Packaged header equals source public header.
- [ ] **SK-12-056** Packaged library exports exactly the expected ABI surface.
- [ ] **SK-12-057** RPATH/runtime dependencies work outside the Vespa source/build tree.
- [ ] **SK-12-058** Clean consumer can link/run without Vespa source checkout or dev container.
- [ ] **SK-12-059** Sanitizer SDK is clearly separated from production SDK artifacts.

---
