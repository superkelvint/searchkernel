# Test Hardening Plan

**Status:** Active implementation plan  
**Purpose:** Reduce Search Kernel fragility by strengthening tests around persistence, lifecycle/concurrency, native execution, cross-layer semantics, malformed input, result fidelity, and compatibility.

An ordinary checklist checkbox means the corresponding test, corpus case, or CI gate is implemented and reachable on `main`.

For hardened sections with implemented work, a separate **Sensitivity verification** gate tracks deliberate-break evidence. That gate is complete only when the CTO has independently demonstrated that a deliberate regression in the protected behavior makes the relevant test/gate fail. A section is not VERIFIED/CLOSED until its implementation checklist and applicable sensitivity gate are satisfied.

## CTO acceptance ledger

This file is the acceptance reference for test-hardening work.

For any pull request implementing a checklist item here:

- the CTO review must cite the applicable section of `implementation-plan/tests.md`;
- PR descriptions, agent claims, and green CI are evidence, not completion;
- ordinary checklist items are checked when the CTO confirms the implementation is reachable from `main` and the item itself is satisfied;
- deliberate-regression proof is tracked separately by each section's **Sensitivity verification** checkbox;
- a section is not VERIFIED/CLOSED until the CTO has independently verified the accepted implementation and completed its sensitivity gate;
- partial implementations must leave genuinely unmet checklist items unchecked.

## Recent merged implementation status — 2026-09-22

The following hardening work is now reachable from `main` and has received CTO
implementation acceptance against this plan. These entries record implementation state. Ordinary checklist boxes below now show
that state directly. Deliberate-regression proof is tracked separately by each
section's **Sensitivity verification** gate.

- PR #203 — deterministic lifecycle/state-machine hardening for the P0 lifecycle
  sequence requirements.
- PR #204 — deterministic seeded lifecycle/document model tests, bounded PR budget,
  larger nightly budget, injected failures, and native reopen coverage.
- PR #206 — expanded named canonical result-fidelity corpus, recursive production
  FlatBuffer assertions, Dispatcher traversal, and Rust public projection checks.
- PR #227 — cross-facade semantic differential tests at the canonical IR/snapshot
  boundary, including Rust/portable, dispatcher/gRPC, Python, TypeScript,
  pagination, grouping/facets, persisted schema, round trips, and negative cases.
- PR #230 — canonical result projection invariants plus one generated shared encoded
  result corpus consumed by Rust/Python/TypeScript, strict facet negatives, and
  fixture-drift checking.
- PR #190 — fast PR/main Rust unit lane plus a larger scheduled/manual nightly
  verification lane.
- PR #221 — supplemental structural guard preventing new production SearchRequest
  semantic entry points outside the canonical resolver path.
- PR #145 — persisted resolved-schema snapshot compatibility: versioned canonical
  snapshot replay, snapshot drift rejection, storage/config identity validation, and
  persistence fault regressions.
- PR #176 — explicit manifest-v3 reopen compatibility without reinterpreting portable
  caller intent; unknown manifest versions continue to fail closed.

GitHub-hosted runner allocation is currently failing before checkout for some exact
heads (`steps=[]`, `runner_id=0`). That is not treated as passing evidence and does
not relax the acceptance rule above.


---

# P0 — Persistence compatibility

Persistent indexes are a compatibility boundary. Historical formats must be tested as immutable artifacts, not reconstructed on the fly.

> **Implemented on main:** PR #145 added versioned resolved-schema snapshot replay and
> compatibility/fault regressions; PR #176 restored explicit manifest-v3 reopen support;
> PR #290 added complete checked-in v2/v3/v4 compatibility fixtures, provenance/version
> coupling, append-only enforcement, the native reopen/search/mutate/reopen verifier,
> expanded fault-matrix regressions, and a dedicated persistence compatibility workflow.
>
> **CTO verification:** PR #290 exact head `34cd6818` passed persistence compatibility,
> Rust unit, behavioral, and client-capability CI; merged main `85ac7683` passed the
> persistence compatibility and Rust unit push gates. Deliberate-regression PR #374
> proved that removing claimed v3 support fails the fixture/version gate and that
> modifying an already-landed v3 fixture fails the append-only gate.
>
> **Still open:** current repository policy intentionally does not execute real-Vespa
> native product tests on GitHub runners. The native historical reopen/search/mutation
> verifier, newly added native fault-matrix cases, and config/snapshot sensitivity
> harness therefore remain unchecked until they are independently executed on an
> exact accepted head.

## Historical fixture corpus

- [x] Check in a real persisted index fixture for every supported historical manifest/storage generation.
- [x] Include at minimum the current manifest format and every prior format that `IndexRuntime::open` claims to support.
- [x] Record fixture provenance: Search Kernel revision, manifest version, storage generation, native ABI version, Vespa revision, schema, mutations, and expected queries.
- [x] Treat historical fixtures as immutable once committed.
- [ ] Add a verifier that opens every historical fixture using current HEAD.
- [ ] Verify search before mutation for each historical fixture.
- [ ] Mutate each compatible fixture, close, reopen, and verify search again.
- [ ] Verify unsupported future manifest versions return `IncompatibleIndex`.
- [ ] Verify unsupported old manifest/storage generations return the documented compatibility error.

## Persisted-state fault matrix

- [x] Missing `manifest/index.json` is classified correctly.
- [ ] Truncated `index.json` is classified correctly.
- [x] Invalid manifest JSON is classified correctly.
- [ ] Missing canonical definition BFBS is classified correctly.
- [ ] Missing portable definition BFBS is classified correctly.
- [x] Corrupted canonical definition BFBS is classified correctly.
- [x] Corrupted portable definition BFBS is classified correctly.
- [ ] Missing resolved schema snapshot is rejected where required.
- [x] Mismatched resolved schema snapshot is rejected.
- [x] Mismatched storage compatibility generation is rejected.
- [x] Mismatched resolved engine configuration identity is rejected.
- [x] Missing generated engine configuration files are rejected.
- [ ] Partially written temporary manifest files cannot be mistaken for committed state.
- [ ] Repeated failed open attempts do not mutate or destroy a valid persisted index.

## CI

- [x] Add a persistence compatibility CI job.
- [x] Run it for changes touching manifest, schema, persistence, API/IR schema encoding, Vespa configuration, or index lifecycle.
- [ ] Run the complete persistence compatibility corpus on `main`.

**Sensitivity verification**

- [x] Deliberately removing support for a claimed historical manifest/storage generation makes the persistence compatibility gate fail.
- [ ] Deliberately weakening persisted-state corruption/config/snapshot validation makes the persistence fault-matrix gate fail.

**Exit criterion:** deliberately removing support for any claimed historical persisted format makes CI fail.

---

# P0 — Lifecycle and state-machine hardening

Lifecycle correctness must be tested over transitions and operation sequences, not only isolated examples.

## Deterministic lifecycle tests

> **Implemented on main:** PR #203. CTO review accepted the architecture and mapped
> the merged tests to this deterministic lifecycle section.

- [x] Closing a runtime invalidates every cloned handle.
- [x] Close waits for one in-flight operation.
- [x] Close waits for multiple simultaneous in-flight operations.
- [x] Operations beginning after close starts are rejected deterministically.
- [x] Double close has defined and tested behavior.
- [x] Drop without explicit close has defined and tested behavior.
- [x] Backend operation failure releases lifecycle accounting.
- [x] Backend panic/unwind cannot permanently wedge close where unwind is supported.
- [x] Failed open leaves no usable partial runtime.
- [x] Failed create leaves no usable partial runtime.
- [x] Repeated open/close cycles do not leak runtime state.
- [x] Concurrent cloned-handle reads remain valid until close.
- [x] Mutations racing with close obey one documented outcome.

**Sensitivity verification**

- [ ] Deliberately breaking lifecycle admission/drain/close semantics makes the deterministic lifecycle gate fail.

## Model/state-machine tests

> **Implemented on main:** PR #204. The merged model covers deterministic seeded
> lifecycle/document sequences with Recording/Scripted backends, injected failures,
> native reopen coverage, a bounded PR budget, and a larger nightly budget.

Build a small reference model for lifecycle/document state. It should model API state and document existence, not Vespa ranking.

Generated actions should include:

```text
create
open
clone handle
insert
upsert
patch
update
delete
get
search
close
drop handle
reopen
attempt operation on closed handle
```

- [x] Implement a deterministic seeded state-machine test.
- [x] Generate long operation sequences against Recording/Scripted backends.
- [x] Compare every operation outcome with the reference model.
- [ ] Persist failing seeds as regression tests.
- [x] Add native-backed state-machine scenarios for operations practical against real Vespa.
- [x] Include reopen transitions in native-backed sequences.
- [x] Include injected failures between lifecycle transitions.
- [x] Run a bounded state-machine suite on every PR.
- [x] Run a larger seed/sequence budget on `main` or nightly.

**Sensitivity verification**

- [ ] Deliberately breaking a modeled lifecycle/document transition makes the seeded model gate fail.

**Exit criterion:** lifecycle regressions are caught through invariant checking across sequences.

---

# P0 — Canonical result fidelity

`SearchResult.root` is canonical. `hits`, `groups`, `facets`, and `aggregations` are projections only.

## Shared canonical result corpus

> **Implemented on main:** PR #206 supplies the expanded named Rust/dispatcher
> corpus; PR #230 supplies the generated shared encoded corpus consumed by first-class
> Python and TypeScript decode paths. CTO review accepted both implementations against
> this section.

Create immutable result fixtures covering:

- [x] root hits only;
- [x] one group containing hits;
- [x] nested groups;
- [x] groups containing both hit and group children;
- [x] multiple sibling groups preserving order;
- [x] group fields and labeled aggregations;
- [x] canonical facet-shaped grouping;
- [x] non-facet grouping that must not be exposed as a facet;
- [x] continuation tokens;
- [x] group limits and metadata;
- [x] coverage;
- [x] timing;
- [x] search-result errors;
- [x] highlights;
- [x] match features;
- [x] summary features;
- [x] rank features;
- [x] sort values;
- [x] finite relevance;
- [x] positive infinity relevance;
- [x] negative infinity relevance;
- [x] absent optional fields;
- [x] present-but-empty optional fields.

## Projection invariants

> **Implemented on main:** PR #230 makes `SearchResult.root` the canonical source
> for Rust/Python/TypeScript projections, adds strict positive/negative facet-shape
> coverage, preserves recursive ordering/state and result metadata, and makes the
> first-class clients consume the same generated fixture bytes. PR #206 adds
> complementary explicit Rust corpus cases.

- [x] Rust canonical tree decoding is lossless.
- [x] Rust `hits` is derived only from the canonical tree.
- [x] Rust `groups` is derived only from the canonical tree.
- [x] Facet projection recognizes only the exact canonical facet shape.
- [x] Aggregation projection recognizes only defined canonical aggregation shapes.
- [x] Convenience projection never reorders children.
- [x] Convenience projection never drops canonical nodes.
- [x] Convenience projection never alters relevance, features, errors, or metadata.
- [x] Python decodes the shared fixtures with equivalent meaning.
- [x] TypeScript decodes the shared fixtures with equivalent meaning when first-class.
- [x] All first-class clients consume the same shared fixtures rather than re-authoring them independently.

**Sensitivity verification**

- [ ] Deliberately loosening facet projection heuristics is detected by the shared result-fidelity tests.
- [ ] Deliberately breaking another canonical projection invariant (ordering/root preservation/metadata) makes the shared result-fidelity gate fail.

**Exit criterion:** changing result projection heuristics incorrectly causes shared result-fidelity tests to fail.

---

# P0 — Differential semantic tests

Whenever two public paths express the same semantics, compare them at the canonical IR boundary.

> **Implemented on main:** PR #227 adds the required cross-facade comparisons at the
> canonical owned-IR/snapshot boundary; PR #221 adds a structural guard against new
> duplicate SearchRequest semantic entry points. CTO review accepted #227 against the
> equivalence matrix below.

## Required equivalence pairs

- [x] Textual filter vs structured filter.
- [x] Repeated filter clauses vs canonical logical `All`.
- [x] Facet helper vs canonical grouping tree.
- [x] Root aggregation helper vs canonical grouping output.
- [x] `Group.by(...)` convenience vs canonical grouping tree.
- [x] `page(n, size)` vs equivalent offset/limit.
- [x] Rust fluent builder vs equivalent portable FlatBuffer request.
- [x] Direct portable request vs gRPC-carried portable request.
- [x] Python facade request vs canonical expected IR.
- [x] TypeScript facade request vs canonical expected IR when first-class.
- [x] Persisted resolved schema decode vs expected schema snapshot.
- [x] API → IR → API → IR preserves canonical meaning for round-trippable values.

## Negative equivalence

- [x] Equivalent invalid forms return equivalent portable error codes.
- [x] Presence-sensitive absent vs present-empty values remain distinguishable where required.
- [ ] Unknown/unsupported capabilities fail at the same semantic boundary regardless of facade/transport.
- [x] No transport-specific defaults are observable.
- [ ] No language-specific semantic defaults are observable.

**Sensitivity verification**

- [ ] Deliberately diverging one alternate representation from canonical IR makes the differential semantic gate fail.

**Exit criterion:** every alternate representation is proven equivalent rather than locally tested in isolation.

---

# P1 — Fuzzing and adversarial input

Short deterministic replay/smoke fuzzing belongs in CI. Larger campaigns may run nightly.

## Dispatcher / FlatBuffers

- [ ] Fuzz arbitrary request bytes through the canonical dispatcher.
- [ ] Fuzz valid envelopes with mutated union discriminators.
- [ ] Fuzz truncated buffers.
- [ ] Fuzz malformed nested values.
- [ ] Fuzz extreme vector/string lengths within harness limits.
- [ ] Assert malformed data cannot execute the backend.
- [ ] Assert no panic, abort, UB, or uncontrolled allocation.

## Filter parser and binder

- [ ] Fuzz arbitrary UTF-8 filter source.
- [ ] Fuzz deeply nested parentheses and NOT expressions.
- [ ] Fuzz malformed operators and token boundaries.
- [ ] Fuzz numeric literals including overflow and exponent extremes.
- [ ] Fuzz bindings independently from source.
- [ ] Assert byte/token/depth/node/IN/binding limits are enforced.
- [ ] Assert recursive descent never exceeds the checked nesting bound.
- [ ] Persist minimized failures as normal regression tests.

## Manifest/persistence reader

- [ ] Fuzz manifest JSON structure.
- [ ] Fuzz canonical and portable BFBS sidecars independently.
- [ ] Fuzz manifest version/storage generation/snapshot combinations.
- [ ] Assert malformed persisted input never mutates disk.
- [ ] Assert malformed persisted input never panics.

## Result decoding

- [ ] Fuzz malformed native result shapes.
- [ ] Fuzz unknown value kinds.
- [ ] Fuzz invalid UTF-8 from native boundaries where representable.
- [ ] Fuzz nested group/result structures.
- [ ] Assert partial decode failures clean up safely.

**Exit criterion:** every untrusted parser/decoder boundary has an adversarial-input harness.

---

# P1 — Property testing

Use property tests where invariants are stronger than example cases.

## Portable semantics

- [ ] API → IR → API → IR is stable for generated supported values.
- [ ] Schema snapshot generation is deterministic.
- [ ] Equivalent normalized schemas produce stable snapshots.
- [ ] Encode → decode preserves supported portable values.
- [ ] Presence-sensitive fields preserve absent vs present-empty semantics.
- [ ] Numeric coercion accepts only lossless legal conversions.
- [ ] Integer coercion always rejects fractional values.
- [ ] Non-finite floats are always rejected where forbidden.
- [ ] Filter clause-local bindings can never leak between clauses.
- [ ] Pagination arithmetic never overflows.
- [ ] Valid page/offset/limit combinations normalize deterministically.

## Query/grouping semantics

- [ ] Boolean query/filter normalization preserves tree meaning.
- [ ] Grouping encode/decode preserves child ordering.
- [ ] Facet convenience always lowers to the documented grouping shape.
- [ ] Duplicate aliases/fields/operators are rejected deterministically.

**Exit criterion:** high-dimensional pure semantics are defended by generated invariants, not only handpicked examples.

---

# P1 — Native C++ / Rust boundary hardening

The native boundary has the highest consequence per bug and must receive targeted negative-path coverage.

## C++ adapter contract tests

- [ ] Every public accessor is tested with null/invalid handles where allowed by the ABI.
- [ ] Out-of-range result/hit/group access is tested.
- [ ] Invalid type access is tested.
- [ ] Invalid UTF-8 behavior is tested where bytes can cross the ABI.
- [ ] Native error objects are tested for ownership and lifetime.
- [ ] Native result objects are tested for ownership and lifetime.
- [ ] Exceptions cannot escape the C ABI.
- [ ] Repeated create/free cycles are tested.
- [ ] Query object lifetime is tested across all required dependent objects.
- [ ] Partial native construction failure cleans up all owned resources.

## Rust native decoding

- [ ] Fault-injection tests cover null native result pointers.
- [ ] Fault-injection tests cover unknown native value kinds.
- [ ] Fault-injection tests cover invalid lengths/counts.
- [ ] Fault-injection tests cover malformed summary fields.
- [ ] Fault-injection tests cover malformed match/rank features.
- [ ] Fault-injection tests cover malformed grouping results.
- [ ] Partial decode failure releases native resources.

## Sanitizers

- [ ] ASan native-contract lane.
- [ ] UBSan native-contract lane.
- [ ] TSan lane for supported concurrency tests, or a documented alternative if Vespa makes TSan impractical.
- [ ] Sanitizer failures block the scheduled hardening gate.

**Exit criterion:** native accessor, ownership, error, and decode failure paths have direct tests rather than incidental coverage.

---

# P1 — gRPC and daemon hardening

Transport must remain semantic-free but operationally robust.

## Request/response behavior

- [ ] Malformed FlatBuffer frame is rejected before dispatch.
- [ ] Wrong FlatBuffer root is rejected before dispatch.
- [ ] Unsupported protocol version is rejected before backend execution.
- [ ] Request size limit is enforced.
- [ ] Response size limit is enforced.
- [ ] Portable operation errors remain portable payload errors.
- [ ] gRPC failures remain transport errors.

## Socket/lifecycle behavior

- [ ] UDS bind/start/stop/restart lifecycle is tested.
- [ ] Stale UDS path handling is tested.
- [ ] Client disconnect during request is tested.
- [ ] Cancellation during in-flight execution is tested.
- [ ] Server shutdown with in-flight requests is tested.
- [ ] TCP fallback start/stop path is tested where supported.
- [ ] Repeated daemon restart does not leak socket/filesystem state.
- [ ] Concurrent clients are tested.

## Semantic firewall

- [ ] Direct portable dispatch and RPC-carried dispatch produce identical IR.
- [ ] gRPC handler contains no request defaults or portable validation.
- [ ] Adding a portable operation does not require protobuf domain changes.

**Exit criterion:** transport failures cannot create divergent Search Kernel semantics.

---

# P1 — Cross-language conformance

First-class SDKs must prove semantic parity against one shared corpus.

- [ ] Define a shared canonical request corpus consumed by Rust and all first-class SDKs.
- [ ] Define a shared canonical response corpus consumed by all first-class SDKs.
- [ ] Rust facade → canonical IR conformance.
- [ ] Python facade → canonical IR conformance.
- [ ] TypeScript facade → canonical IR conformance when first-class.
- [ ] Go facade → canonical IR conformance when first-class.
- [ ] PHP facade → canonical IR conformance when first-class.
- [ ] Cross-language portable error mapping uses the same machine-readable codes.
- [ ] Cross-language result decoding preserves recursive tree meaning.
- [ ] No SDK maintains an independent default table.
- [ ] No SDK maintains an independent filter parser.
- [ ] No SDK maintains an independent facet/grouping semantic implementation.

**Exit criterion:** equivalent language calls demonstrably converge on the same canonical IR and result meaning.

---

# P1 — Real-engine integration coverage

Mocks and RecordingBackend are useful, but integration acceptance must exercise Vespa.

- [ ] Rust direct create/open against real Vespa.
- [ ] Insert/upsert/get/delete against real Vespa.
- [ ] Patch/update against real Vespa.
- [ ] Lexical search against real Vespa.
- [ ] Structured filters against real Vespa.
- [ ] Sorting against real Vespa.
- [ ] Projection/docsums against real Vespa.
- [ ] Pagination and total-hits behavior against real Vespa.
- [ ] Facets/grouping against real Vespa.
- [ ] Ranking/query inputs against real Vespa where supported.
- [ ] Close/reopen/search against real Vespa.
- [ ] Python gRPC runs the same representative real-engine scenario.
- [ ] Rust direct and Python gRPC outcomes are compared for semantic equivalence.
- [ ] Additional first-class SDKs join the same real-engine scenario.

**Exit criterion:** the supported product path has a compact but representative end-to-end real-engine acceptance suite.

---

# P2 — Coverage measurement

Coverage is a diagnostic, not the definition of correctness.

- [ ] Add `cargo llvm-cov` reporting for the Rust workspace.
- [ ] Produce per-crate coverage.
- [ ] Produce branch coverage where practical.
- [ ] Run portable conformance tests under coverage.
- [ ] Run native-feature Rust tests under a separate coverage report where supported.
- [ ] Track critical modules separately: manifest, lifecycle/session, dispatcher, result projection, API↔IR, IR adapter.
- [ ] Add changed-line coverage reporting.
- [ ] Prevent large unexplained coverage regressions on critical modules.
- [ ] Do not use one repository-wide line percentage as the sole merge gate.

**Exit criterion:** coverage reports identify unexercised critical branches without incentivizing meaningless test inflation.

---

# P2 — Mutation testing

Mutation testing should determine whether tests actually detect semantic breakage.

Start with:

```text
searchkernel-ir
searchkernel-filter
searchkernel-dispatch
searchkernel result projection
searchkernel manifest validation
```

- [ ] Add `cargo-mutants` configuration.
- [ ] Establish an initial mutation baseline.
- [ ] Record surviving mutants in critical modules.
- [ ] Add tests for high-risk surviving mutants.
- [ ] Run a targeted mutation subset on PRs touching critical pure-Rust modules if runtime is acceptable.
- [ ] Run the broader mutation suite on a scheduled basis.
- [ ] Treat newly introduced surviving mutants in critical branches as test debt.

**Exit criterion:** critical validation and conversion code is proven sensitive to plausible implementation mistakes.

---

# P2 — Long-running native hardening

Run expensive stress checks outside the fast PR loop but continuously on `main`.

- [ ] Repeated create/open/close loop.
- [ ] Long mutation/search loop.
- [ ] Reopen loop over persistent state.
- [ ] Concurrent search workload.
- [ ] Concurrent mutation/search workload for supported concurrency semantics.
- [ ] File-descriptor count before/after.
- [ ] Thread-count trend before/after.
- [ ] RSS/PSS trend during long runs.
- [ ] Disk-growth trend during repeated mutations.
- [ ] Crash/abrupt termination followed by reopen where supported.
- [ ] Native error-injection stress scenarios.
- [ ] Store logs/artifacts for failed scheduled runs.

**Exit criterion:** leaks, lifecycle degradation, and long-run corruption are continuously checked rather than deferred to release time.

---

# CI structure

> **Current implementation note:** PR #190 split the ordinary Rust unit lane from a
> larger scheduled/manual nightly lane, including broader all-target, contract,
> generated-artifact, client, ABI, and native-cache checks. This is partial progress
> against the CI structure below; unchecked items are not implied complete.

## Fast mandatory PR gate

- [ ] Formatting.
- [ ] Compile/check all targets needed for ordinary development.
- [x] Workspace unit tests.
- [x] Pure semantic differential tests.
- [x] Bounded state-machine tests.
- [x] Shared result-fidelity corpus.
- [ ] Portable request/response conformance.
- [x] Python/client contract checks.
- [ ] Short deterministic fuzz/replay corpus.
- [ ] Critical persistence compatibility smoke set.

## Native/integration PR gate

Trigger for native, persistence, schema/config, adapter, result-decoding, or daemon changes.

- [ ] Real native contract tests.
- [ ] Real Vespa integration scenarios.
- [ ] Historical persistence fixtures.
- [ ] Rust direct vs gRPC semantic comparison where applicable.

## Main/nightly hardening

- [ ] Full historical persistence corpus.
- [x] Large state-machine seed budget.
- [ ] Fuzzing campaigns.
- [ ] ASan.
- [ ] UBSan.
- [ ] TSan or documented concurrency-sanitizer substitute.
- [ ] Mutation testing.
- [ ] Long-running resource/leak tests.
- [ ] Full real-engine cross-entry-point E2E.

---

# Definition of done

This hardening plan is complete when:

- [ ] Every claimed persisted format is protected by a historical reopen fixture.
- [x] Lifecycle correctness is tested through generated operation sequences.
- [ ] Every alternate semantic representation has a differential-equivalence test.
- [x] The canonical result tree has a shared recursive cross-language corpus.
- [ ] All untrusted parser/decoder boundaries have fuzz coverage.
- [ ] High-dimensional pure semantics have property tests.
- [ ] Native ownership/error/decoding paths have direct negative-path coverage.
- [ ] Sanitizers run continuously.
- [ ] Rust direct and gRPC real-engine behavior share representative E2E scenarios.
- [ ] Coverage and mutation testing are used to expose weak tests.
- [x] Fast PR checks and expensive scheduled hardening are clearly separated.
- [ ] A deliberate regression in each critical boundary has been shown to make the relevant gate fail.
