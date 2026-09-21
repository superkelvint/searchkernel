# 18. Capability registry and documentation truthfulness

> **Modular checklist:** [Index](README.md) · [Evidence model and repository-wide rules](00-audit-method.md) · [Audit record and freeze rules](19-audit-record-and-freeze.md)


**Minimum evidence to PASS this section:** `SRC + STATIC + VER/E2E` for each support claim. `DOC` alone is forbidden as capability evidence.

- [ ] **SK-18-001** `docs/capabilities.json` is itself complete enough to be an audit instrument: every materially advertised portable capability has a row or an explicit documented reason for exclusion.
- [ ] **SK-18-002** Running `python3 tools/check-capabilities.py` from a pristine copy succeeds. In this snapshot it currently reports the generated capability documents stale because `LIMITATIONS.md` is absent; do not silently run `--write` and then call the source export clean.
- [ ] **SK-18-003** The ledger language dimension matches first-party clients in the tree. In this snapshot `tools/check-capabilities.py` hard-codes exactly `rust` and `python`, so TypeScript ergonomic/parity claims cannot be represented and require separate evidence until the ledger schema is extended.
- [ ] **SK-18-004** Every row in `docs/CAPABILITIES.md` accurately distinguishes represented / semantic / executable / ergonomic status.
- [ ] **SK-18-005** Capability claims cite or point to concrete verifier/test evidence.
- [ ] **SK-18-006** Known limitations are not contradicted by README/examples/client docs.
- [ ] **SK-18-007** Unsupported high-level facade gaps are documented without implying low-level protocol support equals ergonomic support.
- [ ] **SK-18-008** `filter.structured` / `filter.textual` status matches each facade.
- [ ] **SK-18-009** Query variant status matches Rust/Python/TypeScript facade availability separately from backend execution.
- [ ] **SK-18-010** Search sort/pagination/search-field limitations match actual adapter/native support.
- [ ] **SK-18-011** Docsums/highlighting/rich-value/summary-feature capability status matches actual result seam.
- [ ] **SK-18-012** Grouping structured/functions/aggregations/summary/continuation/raw-source status matches actual semantic and native paths. In this snapshot `grouping.summary_aggregation` is still marked `executable: false` with a claim that grouped docsums do not cross the native ABI, while current source contains safe-engine grouped summary decoding, adapter `HitList -> ResultNode::Hit` mapping, and Python E2E exact nested-hit assertions. Treat this as a concrete ledger inconsistency requiring execution/reconciliation, not an open-ended guess.
- [ ] **SK-18-013** Rich schema status covers document summaries, imported/search-only fields, fieldsets, constants, ONNX models, rank functions/phases/features, and inheritance; these cannot disappear from the capability story merely because native lowering rejects them.
- [ ] **SK-18-014** Update operation status distinguishes represented vs IR-supported vs executable vs facade-supported.
- [ ] **SK-18-015** Tensor capability status distinguishes dense document CRUD, query inputs, feature readback, sparse/mixed support.
- [ ] **SK-18-016** Ranking capability status distinguishes profile selection, numeric/tensor inputs, rank properties, feature outputs, and two-phase execution. The current ledger has no dedicated two-phase-ranking capability row despite Rust/Python E2E source exercising it; add/justify the omission rather than hiding it under generic profile selection.
- [ ] **SK-18-017** TypeScript rank-profile ergonomics are represented independently; current `RankProfile` sugar lacks second-phase/rerank-count setters even though the portable/native stack supports two-phase execution elsewhere.
- [ ] **SK-18-018** Embedded scope correctly excludes distributed/container facilities.
- [ ] **SK-18-019** `docs/status.md`, README, architecture, roadmap, implementation plan, and capability registry do not materially contradict the current tree. In this snapshot `docs/status.md` inventories Python but omits the present TypeScript client, while its generic non-Rust architecture prose says generated gRPC clients even though the Node transport is hand-written HTTP/2; reconcile this before treating status docs as evidence.

---

---

# 19. Build, packaging, generated-state, and clean-environment reproducibility

**Minimum evidence to PASS this section:** `PKG + GEN` from pristine state; native package claims additionally require link/run verification against the exact SDK artifact.

- [ ] **SK-19-001** `cargo fmt --check` passes on a clean checkout.
- [ ] **SK-19-002** Workspace Clippy passes with the repository-required warning policy.
- [ ] **SK-19-003** `cargo test --workspace` passes from a clean source checkout/export after the documented bootstrap steps; missing generated Rust protocol/RPC include targets are treated as a source-distribution defect unless generation is automatically invoked.
- [ ] **SK-19-004** Native SDK/header/bindings generation checks pass before Rust native tests.
- [ ] **SK-19-005** Python install/import/tests pass in a fresh virtualenv with only declared dependencies.
- [ ] **SK-19-006** TypeScript generate/typecheck/build/test/pack/install/import pass from a clean directory.
- [ ] **SK-19-007** No client or Rust crate requires undeclared generated files that happen to exist in a developer's working tree.
- [ ] **SK-19-008** A clean source export is tested with generated directories deleted first, then only documented generation/build commands; pre-existing `dist/`, `.example-dist/`, `.pytest_cache/`, `*.egg-info`, `target/`, and generated trees cannot satisfy the gate.
- [ ] **SK-19-009** No verifier depends on `.pytest_cache`, `target/`, `dist/`, generated clients, stale sockets, or persisted indexes from a previous run unless explicitly part of its fixture.
- [ ] **SK-19-010** Clean-state scripts delete/recreate only test-owned state.
- [ ] **SK-19-011** Toolchain versions (`rustc`, C/C++, FlatBuffers, protobuf/grpc generators, Node, Python where pinned) are recorded/validated where reproducibility requires them.
- [ ] **SK-19-012** Native shared-library dependencies are packageable on the supported target and do not accidentally bind to build-host-only libraries.
- [ ] **SK-19-013** Package manifests include all generated/runtime files required by end users.

---

---

# 20. Cross-language semantic parity

**Minimum evidence to PASS this section:** `CONF + E2E` across every claimed first-party entry path using the same scenario IDs/expected semantics.

**Current snapshot warning:** no single authoritative verifier in this tree proves Rust direct + Python gRPC + TypeScript gRPC semantic parity end-to-end. Treat the items below as a required gate to build/run, not as something implied by `reference/vespa/i` or Phase 10.

- [ ] **SK-20-001** Equivalent Rust/Python/TypeScript high-level calls for the **overlapping ergonomic surface** produce equivalent canonical IR, allowing language-idiomatic syntax differences only; missing sugar in one language is recorded as a capability gap rather than forced into false equivalence.
- [ ] **SK-20-002** Explicit `0`, `false`, empty list, empty projection, and absent values retain the same meaning across languages.
- [ ] **SK-20-003** Int64/UInt64 values preserve full range across languages.
- [ ] **SK-20-004** Tensor values preserve kind/type/data across languages.
- [ ] **SK-20-005** Filter bindings are clause-local in every client.
- [ ] **SK-20-006** Pagination math and overflow behavior are equivalent.
- [ ] **SK-20-007** Facet/grouping conveniences normalize to equivalent canonical grouping.
- [ ] **SK-20-008** Query annotations and typed query variants produce equivalent IR.
- [ ] **SK-20-009** Ranking profile/input/property namespaces remain equivalent.
- [ ] **SK-20-010** Recursive result trees decode with the same semantic content.
- [ ] **SK-20-011** Infinite relevance/non-finite portable values preserve equivalent meaning.
- [ ] **SK-20-012** Structured portable errors expose equivalent codes/details without message parsing.
- [ ] **SK-20-013** No client adds semantic defaults absent from central IR.

---

---

# 21. End-to-end product invariants

**Minimum evidence to PASS this section:** `E2E` through the real product paths; durability/resource/concurrency invariants additionally require `HARD`.

- [ ] **SK-21-001** Create an index through the public product definition and reopen it from disk successfully.
- [ ] **SK-21-002** Insert/upsert/get/patch/update/delete operate on the same durable native data.
- [ ] **SK-21-003** Text indexing and query linguistics use compatible analyzed terms/positions.
- [ ] **SK-21-004** Structured filters compose correctly with text/typed/YQL query sources where allowed.
- [ ] **SK-21-005** Sorting, pagination, projection, docsums, total hits, and ranking operate on the actual matched set.
- [ ] **SK-21-006** Facets/grouping/aggregations are computed by Vespa and correspond to the same query/filter scope as hits.
- [ ] **SK-21-007** Summary aggregation returns the exact expected nested hit IDs and fields.
- [ ] **SK-21-008** Ranking profile selection, query inputs, rank properties, first phase, and second phase alter results/features as expected.
- [ ] **SK-21-009** Tensor document values survive close/reopen and can participate in vector queries where supported.
- [ ] **SK-21-010** Approximate ANN demonstrably uses the configured HNSW/approximate path when requested, with a separate exact path/control.
- [ ] **SK-21-011** Coverage/degradation metadata is accurate when result quality is intentionally degraded or hard failure occurs.
- [ ] **SK-21-012** Close/reopen produces equivalent search behavior after all supported mutations.
- [ ] **SK-21-013** Rust direct and non-Rust gRPC paths produce equivalent real-engine outcomes for the same portable intent, proven by a dedicated cross-entry-path gate rather than inferred from the Rust-only I-series verifier.
- [ ] **SK-21-014** Malformed portable/gRPC/native inputs fail safely without process corruption.
- [ ] **SK-21-015** Long-running mixed mutation/search/grouping/ranking workload does not leak unbounded memory, threads, file descriptors, sockets, or disk state.

---

---

# 22. Adversarial semantic-bypass search

**Minimum evidence to PASS this section:** `STATIC` search outputs plus targeted `UNIT/NEG/E2E` regression tests for every discovered bypass. Grep alone cannot close a finding.

- [ ] **SK-22-001** Search production source for any direct portable/client -> Vespa/native call bypassing `*_to_ir()`.
- [ ] **SK-22-002** Search production source for duplicate implementations of defaults or field/operator legality outside `searchkernel-ir`.
- [ ] **SK-22-003** Search clients for parsing of textual filter syntax.
- [ ] **SK-22-004** Search clients for hand-written gRPC framing/HTTP2 protocol logic.
- [ ] **SK-22-005** Search adapter/native code for Rust-side matching, BM25, ranking, filtering, or aggregation fallbacks.
- [ ] **SK-22-006** Search for ignored `Unsupported` results or `unwrap_or/default` logic that silently degrades unsupported requests.
- [ ] **SK-22-007** Search for truthiness-based defaults (`or 10`, `|| 10`, etc.) that overwrite valid zero/false values.
- [ ] **SK-22-008** Search result conversions for wildcard/default union handling that treats unknown variants as a known variant.
- [ ] **SK-22-009** Search value conversions for content-based type inference that can confuse bytes/lists or tensor kinds.
- [ ] **SK-22-010** Search generated-artifact paths for source/header/schema drift that ordinary builds do not catch.
- [ ] **SK-22-011** Search verifier scripts for commands that regenerate expected results before comparing them.
- [ ] **SK-22-012** Search tests for fixture-specific IDs/values mirrored in production source.
- [ ] **SK-22-013** Search production Rust outside bindings for raw FFI declarations/calls.
- [ ] **SK-22-014** Search production C++ outside native adapter for Vespa-specific integration leaking elsewhere.
- [ ] **SK-22-015** Search repository for active references to retired portable FFI/native client transport architecture.
- [ ] **SK-22-016** Search schema/IR adapters for broad `Unsupported` guards that accidentally reject a subfeature whose lower layer now implements it, or conversely allow a represented feature to be dropped before the adapter can reject it.
- [ ] **SK-22-017** Search capability/docs generation for dimensions absent from the current first-party surface (notably TypeScript) and for evidence paths that exist but do not actually test the claimed behavior.

---

---

# 23. Required final audit gates

**Minimum evidence to PASS this section:** Every gate here requires a recorded command, revision/digest, exit status, and artifact/log reference. No gate may be inferred from another gate.

A repository-wide audit is complete only when all of the following are checked:

- [ ] **SK-23-001** Portable schema/BFBS/generated-code freshness gate passes.
- [ ] **SK-23-002** Standard FlatBuffers JSON tests pass; `reference/json-conformance` is not counted until it contains real frozen fixtures.
- [ ] **SK-23-003** `searchkernel-filter` **parser/lexer/complexity** golden conformance passes.
- [ ] **SK-23-004** `searchkernel-ir` **schema-aware textual-filter binder/equivalence** tests pass separately.
- [ ] **SK-23-005** `searchkernel-ir` semantic and round-trip conformance passes.
- [ ] **SK-23-006** Public Rust facade/testkit semantic tests pass.
- [ ] **SK-23-007** Dispatcher malformed/version/operation/conformance tests pass.
- [ ] **SK-23-008** IR adapter focused unit/integration tests pass.
- [ ] **SK-23-009** Native ABI/header/export/bindings/SDK freshness checks pass.
- [ ] **SK-23-010** Vespa engine unit/native contract tests pass.
- [ ] **SK-23-011** M-series cumulative verifier passes through the active baseline named by `reference/vespa/README.md` (M13 in this snapshot), including explicit M12 tensor-query/NN and M13 tensor-document coverage.
- [ ] **SK-23-012** Search-reply/recovery/Phase 9/Phase 10 specialized verifiers pass where applicable.
- [ ] **SK-23-013** `searchkerneld` UDS/TCP/gRPC/gRPC-Web tests pass.
- [ ] **SK-23-014** Python generation/package/unit/transport/real-engine E2E gate passes from clean environment.
- [ ] **SK-23-015** TypeScript generation/typecheck/build/unit/package/transport/real-engine E2E gate passes from a source-clean environment, and Node transport uses a standard generated gRPC client rather than manual HTTP/2 framing.
- [ ] **SK-23-016** Cross-language canonical-semantic corpus passes. This is a required new/identified gate if no existing script actually runs all first-party entry paths; do not mark it from documentation claims.
- [ ] **SK-23-017** Rust-direct vs gRPC real-engine conformance passes via a dedicated executable gate; the current Rust-only `reference/vespa/i` verifier is insufficient evidence.
- [ ] **SK-23-018** Sanitizer/hardening/stress/resource-leak gates pass.
- [ ] **SK-23-019** Capability/status documentation is reconciled with the verified evidence, including TypeScript and rich-schema families that the current ledger does not fully model.
- [ ] **SK-23-020** Generated-source bootstrap closure passes for Rust protocol code, Rust RPC code, Python wire/RPC code, and TypeScript wire/RPC code.
- [ ] **SK-23-021** No critical/high finding remains open without being explicitly documented as unsupported/non-release-blocking by the project contract.

---

---

# 24. Snapshot-specific evidence-backed findings and closure checks

**Minimum evidence to PASS this section:** These are evidence-backed known findings for this snapshot. Closure requires the evidence stated in each item plus the affected final gate(s).

These findings are already supported by source-level evidence in this exact archive. The checkboxes below are **closure obligations**, not requests to rediscover the finding. Attach the original source evidence and the remediation/regression evidence before checking them.

- [ ] **SK-24-001** **TypeScript Node transport violates the target transport invariant today.** `clients/typescript/src/transport/grpc-node.ts` hand-writes Node HTTP/2 connection management, 5-byte gRPC framing, `grpc-timeout`, status/trailer handling, unary response parsing, and compression rejection. Replace it with standard generated/runtime gRPC plumbing before marking the architecture invariant complete.
- [ ] **SK-24-002** **TypeScript generated-source bootstrap is not closed.** `src/generated/**` is absent; `npm run build` and `prepack` do not invoke generation. Prove a clean package build without developer-local generated files.
- [ ] **SK-24-003** **Rust generated-source bootstrap is not closed by the checked-in source alone.** `searchkernel-protocol/src/protocol.rs` includes absent `src/generated/**`; `searchkerneld` includes absent `src/generated/rpc.rs`. Verify the normal clean build invokes the documented generators before compilation.
- [ ] **SK-24-004** **FlatBuffers JS runtime/generator versions differ.** TypeScript declares runtime `25.9.23` while generation is pinned to `25.12.19`; prove compatibility or align them.
- [ ] **SK-24-005** **`reference/json-conformance` contains no files.** It is a placeholder, not a frozen corpus.
- [ ] **SK-24-006** **API conformance is narrower than the portable surface.** Do not infer advanced typed-query, full advanced update, recursive result, tensor, or rich-ranking coverage from directory names; inventory actual cases and add missing supported families.
- [ ] **SK-24-007** **Filter parser and binder are separate proof surfaces.** Syntax/parser goldens live in `searchkernel-filter`; schema-aware textual-filter binding/equivalence lives in `searchkernel-ir`.
- [ ] **SK-24-008** **M9 recursive grouping verifier is deliberately narrow.** It does not prove aliases, order, precision, grouping filters/keep, functions, summary hits, continuations, Python, dispatcher, or IR.
- [ ] **SK-24-009** **Phase 9 does not alone prove portable summary hits.** It reaches low-level safe-engine/native summary-hit decoding plus selected adapter tests. Portable nested-hit semantics need adapter/client evidence too.
- [ ] **SK-24-010** **I-series verifier is Rust-only.** `reference/vespa/i/verify-i.sh` runs the public Rust driver after native/Phase10 gates; it does not compare Rust with portable/gRPC clients.
- [ ] **SK-24-011** **Capability generation is stale/incomplete for this source export.** `tools/check-capabilities.py` expects generated `LIMITATIONS.md`, which is absent, and hard-codes only Rust/Python as ergonomic language dimensions.
- [ ] **SK-24-012** **Capability ledger summary-aggregation row contradicts current source evidence.** It says grouped docsums do not cross the native ABI, while current native-engine test, adapter mapping, and Python E2E source implement/assert nested grouped summary hits. Execute the real gate and reconcile the ledger.
- [ ] **SK-24-013** **Capability ledger lacks a dedicated two-phase-ranking row.** Rust/Python E2E source explicitly exercises second-phase reranking; make that support state auditable instead of implicit.
- [ ] **SK-24-014** **TypeScript rank-profile facade is not two-phase ergonomic.** Current `RankProfile` exposes first phase/match features/query inputs, but not second phase/rerank count. Record this separately from backend capability.
- [ ] **SK-24-015** **Status documentation is stale relative to the tree.** `docs/status.md` omits the current TypeScript client and describes non-Rust generated gRPC-client architecture despite the Node handwritten transport.
- [ ] **SK-24-016** **Unix socket ownership remains high risk.** Treat connection failure as insufficient proof of staleness, and ensure shutdown/Drop cannot unlink a replacement socket created after bind.
- [ ] **SK-24-017** **Exported native SDK is incomplete for link/run proof.** `core/native/sdk/linux-x86_64/lib/` is empty in this archive; rebuild/provide the exact library before native acceptance.
- [ ] **SK-24-018** **Rich V2 query/presentation controls need explicit audit coverage.** QueryModel/QueryModelType, PresentationRequest, query-profile/search-chain and generic property collision/applicability behavior are represented in the canonical schema and must be executed or explicitly rejected without disappearing.
- [ ] **SK-24-019** **Rich schema rejection matrix is rechecked against lower-layer progress.** Broad adapter rejection of inheritance, fieldsets, search-only/imported fields, summaries, constants, ONNX and richer rank-profile features cannot hide newly implemented lower-layer support.

---
