# Change recipes

Architecture explains where Search Kernel responsibilities live. This document
explains how a capability moves through those responsibilities.

Use the smallest recipe that matches the requested change. A feature does not
need to reach every layer in one change. Track its progress independently as:

```text
REPRESENTED  FlatBuffers can express it
SEMANTIC     *_to_ir() validates and gives it explicit meaning
EXECUTABLE   the Vespa adapter can execute it
ERGONOMIC    a public SDK facade makes it convenient
```

`REPRESENTED -> SEMANTIC -> EXECUTABLE` is an ordered backend-support chain.
`ERGONOMIC` is tracked independently per language: facade sugar may exist before
backend execution is available, provided execution still returns the canonical
`Unsupported` error. Do not collapse these fields into a single "highest stage".

The stages are not a reason to add speculative layers. First look for an existing
type, IR variant, Vespa primitive, native handle, or generated binding that
already expresses the capability.

## Common gates

Before editing, answer these questions:

1. Is the capability portable, Rust-only, or language-specific ergonomics?
2. Does an existing FlatBuffers union/table, IR type, adapter method, or native
   primitive already represent it?
3. What is the normative behavior: the frozen contract, pinned Vespa source and
   tests, or a supplied oracle?
4. Which positive, malformed, unsupported, and boundary cases prove the change?
5. Does the capability ledger need its represented, semantic, executable, or
   ergonomic stage updated? The source ledger is `docs/capabilities.json`;
   `docs/CAPABILITIES.md` and `LIMITATIONS.md` are generated views.

Keep portable semantic implementation in `searchkernel-ir`. Rust and non-Rust
entry points must converge on the same meaning. `searchkernel-filter` owns only
filter syntax parsing; schema-aware filter meaning still belongs in
`searchkernel-ir`. Do not edit generated protocol/client output by hand, patch
`upstream/vespa/`, or add a native ABI function until the Vespa capability and
boundary need are demonstrated.

### Generated-artifact map

Do not regenerate unrelated transports when the portable domain schema changes.

- For `api/*.fbs` changes, regenerate/check the BFBS, Rust FlatBuffers models,
  and any first-class language FlatBuffers models that consume the changed
  schema. In the current tree that normally means:

  ```bash
  bash tools/generate-flatbuffers-bfbs.sh
  tools/generate-searchkernel-protocol.sh
  tools/generate-python-flatbuffers.sh   # when Python consumes the changed schema
  bash tools/check-flatbuffers-bfbs.sh
  ```

- Do **not** change `api/rpc/searchkernel.proto` or regenerate gRPC stubs merely
  because a Search Kernel domain operation/field changed. The protobuf is a
  bytes-only transport envelope. Regenerate RPC stubs only when that `.proto`
  itself changes; Python uses `tools/generate-python-grpc.sh`.
- Generated files are outputs, never the source of a semantic change.

### Conformance-suite scope

`reference/api-conformance` is frozen specification material and deliberately
does not cover every advanced capability (notably grouping in the current
snapshot). Add a case there only when the suite's contract/README declares the
operation in scope. Otherwise add focused `searchkernel-ir` ingress-equivalence
tests and the capability-specific verifier. Do not widen a frozen conformance
suite ad hoc just to satisfy a recipe.

## Recipe: add a top-level portable operation

Use this when adding a new request/response operation alongside search, get,
update, analyze, describe, or inspect-query. Do not add a new protobuf RPC method.

1. Define the operation's domain request/result tables in `api/searchkernel.fbs`.
2. Append the request and response payload variants to
   `api/searchkernel_request.fbs` and `api/searchkernel_response.fbs`. Preserve
   existing union discriminator values; do not reorder existing variants.
3. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above. The gRPC `.proto` normally remains unchanged.
4. Add the owned request/result representation and one canonical API-to-IR
   semantic path in `searchkernel-ir`. If both the Rust facade and FlatBuffers
   entry point exist, have them converge on the same unresolved input/resolver
   rather than constructing resolved IR independently.
5. If the operation executes a backend capability, extend the `Backend` surface
   and `BackendSession` forwarding. If it is intentionally context-only (like
   describe/inspect), keep that distinction explicit rather than adding a fake
   backend method.
6. Add IR-to-FlatBuffers response encoding under
   `core/crates/searchkernel-ir/src/ir_to_api/`
   and preserve canonical portable errors.
7. Add exactly one route in `core/crates/searchkernel-dispatch/src/lib.rs`:
   envelope payload -> `*_to_ir()` -> backend/context call -> canonical response
   encoder. Keep version/root/unknown-operation handling in the dispatcher.
8. Extend `searchkernel-testkit` recording/scripted support when the operation is
   backend-facing, and add dispatcher tests for success plus malformed payloads.
9. Add direct Rust and non-Rust ergonomic entry points independently. Non-Rust
   clients continue to use the same bytes-only gRPC `Execute` method.
10. Add portable conformance cases when the frozen suite owns the operation, plus
    real-backend acceptance before marking it executable.
11. Update the capability ledger fields/evidence independently and regenerate the
    generated views.

Verify envelope compatibility, API-to-IR semantics, dispatcher routing, response
encoding, and the relevant real-Vespa lane. A gRPC transport test alone does not
prove operation semantics.

## Recipe: add a portable query node

Use this when a query expression must be expressible through the canonical
portable API.

1. Define the node, payload, defaults, and malformed-input behavior in
   `api/searchkernel.fbs`. Append it to the existing `QueryExprKind` union; do
   not reorder existing variants because their FlatBuffers union discriminator
   values are part of the released wire shape.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above. Regenerate language models only for first-class SDKs that consume
   the changed schema.
3. Add FlatBuffers-to-IR decoding in
   `core/crates/searchkernel-ir/src/api_to_ir/query.rs`. For a normal typed-query
   expression, extend `QueryExprKindIr` in
   `core/crates/searchkernel-ir/src/model/query.rs` rather than adding a
   new top-level query source.
4. Extend the shared typed-query semantic validation in
   `core/crates/searchkernel-ir/src/api_to_ir/query.rs`
   (`validate_typed_query` and, where relevant, annotation/default resolution).
   Decoding a union into an IR variant is not sufficient semantic support.
5. Add/update IR-to-FlatBuffers conversion in
   `core/crates/searchkernel-ir/src/ir_to_api/query.rs`. The current InspectQuery
   path serializes resolved queries, so typed-query nodes should normally round
   trip through this path.
6. Add positive, malformed, unsupported, field/type, annotation, and nesting
   cases. Put them in the frozen portable conformance suite only when that suite
   owns the case; otherwise use focused IR ingress-equivalence tests. Assert the
   canonical error for invalid unions and capability mismatches.
7. Lower the typed query through
   `core/crates/searchkernel-ir-adapter/src/search/typed_query.rs` and its
   focused submodules.
8. Inspect the Vespa query model and pinned source. Extend
   `core/crates/searchkernel-vespa-engine/src/query/` only if the existing
   query primitive cannot represent the IR node.
9. Add or change the native ABI only when the engine wrapper lacks the required
   primitive. Follow the native ABI recipe below.
10. Add ergonomic Rust and Python helpers independently; generated FlatBuffers
    model types receive the represented schema shape without requiring facade
    sugar.
11. Update each applicable field in `docs/capabilities.json` independently, then
    regenerate its views with `python3 tools/check-capabilities.py --write`. Do
    not infer ergonomic support from executable support or vice versa.

Verify the shared API conformance suite when the node is in that suite's declared
scope, plus IR/adapter tests, query lowering tests, and the applicable real-Vespa
lane. Do not accept a Rust-only query mock as evidence of executable support.

## Recipe: add a field type

Use this when schema definitions, document values, or field capabilities need a
new type.

1. Define the portable field/value shape and optional-field semantics in
   `api/searchkernel.fbs`.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above.
3. Update schema and value conversion in the relevant
   `core/crates/searchkernel-ir/src/api_to_ir/` modules.
4. Add the owned field/value representation under
   `core/crates/searchkernel-ir/src/model/`, including capability validation and
   deterministic defaults.
5. Add response conversion under
   `core/crates/searchkernel-ir/src/ir_to_api/` if the type can be returned,
   grouped, or exposed in schema results.
6. Trace every operation that consumes the type: document creation, updates,
   search/filter/sort/grouping, summaries, and result decoding. Do not claim a
   field type is supported merely because it parses.
7. Lower it through `core/crates/searchkernel-ir-adapter` and the Vespa
   schema/document adapter. Inspect the pinned Vespa implementation and tests for the actual
   storage, indexing, attribute, and summary behavior.
8. Add positive and negative schema conformance cases, including invalid field
   capability combinations, empty/default values, round trips, and persistence
   reopen where applicable.
9. Add Rust and SDK helpers only after the portable and semantic paths are clear.
   If the Rust schema facade exposes the type, extend its `RustFieldKind` /
   `resolve_rust_index_definition*` path so it resolves to the same `FieldIr`;
   do not duplicate type defaults or capability rules in the facade.
10. Update the applicable `docs/capabilities.json` support fields and evidence,
    then regenerate its views with `python3 tools/check-capabilities.py --write`.

Verify API conformance, IR conversion tests, adapter model-mutation tests, and
the real Vespa schema/document lane required by the field capability.

## Recipe: add an update operator

1. Add the operator and its payload to the canonical update union in
   `api/searchkernel.fbs`.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above.
3. Decode the FlatBuffers operation in
   `core/crates/searchkernel-ir/src/api_to_ir/document.rs`, add/extend the
   corresponding `UpdateOperationInput` in
   `core/crates/searchkernel-ir/src/api_to_ir/resolve/mod.rs`, and add the owned
   operation in `core/crates/searchkernel-ir/src/model/document.rs`.
4. Put field/path validation, field-type legality, value normalization, and
   unsupported-operator behavior in the shared `resolve_update` path in
   `core/crates/searchkernel-ir/src/api_to_ir/resolve/document.rs` so the direct Rust and
   FlatBuffers entry paths converge.
   Keep schema-time unknown-field errors distinct from backend runtime behavior
   for a missing document/value.
5. Add adapter lowering and use the existing Vespa document-update primitive
   when possible. Inspect `core/native/vespa/update.cc` before adding a new
   native entry point.
6. Add direct Rust API construction and Python ergonomic sugar separately from
   the portable operation.
7. Add positive, negative, persistence, and reopen tests through the real
   update path. Include response/error conversion if the operator returns a
   value.
8. Add a native ABI change only if the adapter cannot reach the Vespa primitive
   with the existing boundary.
9. Update the applicable `docs/capabilities.json` support fields, field-type
   limitations, and evidence independently, then regenerate its views with
   `python3 tools/check-capabilities.py --write`.

Verify focused IR, adapter, native CRUD, and cumulative update/persistence
acceptance tests.

## Recipe: add a grouping construct

1. Add the lossless grouping operation, expression, or value to
   `api/searchkernel.fbs`. Preserve structural distinctions that affect Vespa
   grouping semantics; do not encode raw grouping syntax as an untyped field.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above.
3. Add conversion in
   `core/crates/searchkernel-ir/src/api_to_ir/grouping.rs`, the owned form in
   `core/crates/searchkernel-ir/src/model/grouping.rs`, and response conversion
   in `core/crates/searchkernel-ir/src/ir_to_api/grouping.rs` or `value.rs` as
   appropriate.
4. Add semantic validation for nesting, aliases, expression types, ordering,
   limits, and unsupported constructs in a shared IR validation path callable by
   both portable decoding and direct Rust construction. **Current seam:** the
   portable decoder validates much of the structure in
   `core/crates/searchkernel-ir/src/api_to_ir/grouping.rs`, while the Rust
   `GroupingPlan::into_ir()` path can construct `GroupingRequestIr` directly and
   `resolve_grouping()` passes structured IR through. If a new rule must apply to
   both entry points, extract/use a shared validator first; do not add it only to
   the FlatBuffers decoder.
5. Lower through
   `core/crates/searchkernel-ir-adapter/src/search/grouping.rs` and inspect
   the pinned Vespa grouping behavior before extending the engine adapter.
6. Add cases for a minimal valid construct, nested composition, malformed
   unions, invalid operands, and canonical result shape. Use
   `reference/api-conformance` only for cases its frozen contract owns; grouping
   is currently verified primarily by IR/adapter and Vespa grouping lanes.
7. Add Rust/Python facade helpers independently. Keep cross-entry grouping
   validation in `searchkernel-ir`, not in the facade. The current Rust
   `GroupingPlan::into_ir()` directly constructs grouping IR; treat that as an
   existing convenience seam, not precedent for placing new portable semantics
   there.
8. Add or update a real Vespa grouping verifier and source trace when execution
   changes.
9. Update `docs/capabilities.json` and regenerate its views with
   `python3 tools/check-capabilities.py --write`.

Verify IR/portable ingress tests, adapter grouping tests, result decoding, and
the applicable grouping/YQL or cumulative Vespa verifier.

## Recipe: add a result value

1. Define the response value or result metadata in `api/searchkernel.fbs`.
   Change `api/searchkernel_response.fbs` only when the response envelope or
   payload union itself changes. Decide whether the value is a scalar, list,
   structured value, union, or opaque payload.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above.
3. Add the owned result/value type under
   `core/crates/searchkernel-ir/src/model/result.rs` or `model/value.rs`.
4. Decode the backend/native value into IR in the adapter and convert it back
   through `core/crates/searchkernel-ir/src/ir_to_api/result.rs` or `value.rs`.
5. Preserve absence, explicit defaults, ordering, numeric limits, and error
   degradation semantics.
6. Add portable golden cases, including malformed/unsupported values and
   round-trip cases where relevant.
7. Add focused native result-decoding coverage if the value comes from Vespa.
   Extend the native ABI only when the existing result boundary cannot carry it.
8. Add ergonomic Rust/Python result wrappers independently of the canonical
   representation.
9. Update `docs/capabilities.json`, regenerate its views, and verify the actual
   result path.

Verify IR conversion, adapter decoding, protocol/golden conformance, and the
real search/result acceptance lane.

## Recipe: add a ranking setting

1. Add the portable setting and its presence/default semantics to the ranking
   portion of `api/searchkernel.fbs`.
2. Regenerate the applicable FlatBuffers artifacts using the generated-artifact
   map above.
3. If the setting is part of a search request, decode it into the unresolved
   search input in `core/crates/searchkernel-ir/src/api_to_ir/search.rs` and
   resolve/validate it in the shared `resolve_search` path in
   `core/crates/searchkernel-ir/src/api_to_ir/resolve/search.rs`. Add the owned resolved
   form to `core/crates/searchkernel-ir/src/model/ranking.rs`.
4. Keep `core/crates/searchkernel-ir/src/api_to_ir/ranking.rs` consistent for
   standalone ranking conversion/testkit uses, but do not create a second set of
   defaults or validation there. Convert resolved ranking back through
   `core/crates/searchkernel-ir/src/ir_to_api/ranking.rs` wherever inspection or
   canonical round trips expose it.
5. Lower it through the adapter's search/ranking path and inspect the pinned
   Vespa ranking/query-property behavior. Keep request defaults, validation, and
   normalization in `searchkernel-ir`; do not reimplement rank-profile
   evaluation, scoring, BM25, or Vespa ranking execution in Rust.
6. Add valid, default, conflicting, malformed, and unsupported setting cases.
   Use the frozen portable conformance suite only when ranking settings are in
   that suite's declared scope; otherwise use focused IR equivalence tests.
7. Add the direct Rust builder and non-Rust ergonomic helper only as syntax
   sugar over the same IR setting.
8. Add a real Vespa source trace and verifier when execution or score behavior
   changes.
9. Update `docs/capabilities.json` and regenerate its views with
   `python3 tools/check-capabilities.py --write`.

Verify ranking IR tests, adapter lowering, portable goldens, and the applicable
ranking/match-feature Vespa verifier.

## Recipe: change the textual filter language

Use this for syntax, precedence, literals, parameters, or textual-filter binding
behavior. Do not put the language parser in an SDK or the Vespa adapter.

1. Update `docs/filter-expression-language.md` and its frozen fixtures first when
   the public language contract changes.
2. Keep syntax-only lexing/parsing and complexity bounds in
   `core/crates/searchkernel-filter`. That crate must remain schema-independent
   and Vespa-independent.
3. Keep schema lookup, field filterability, type/operator legality, typed binding
   normalization, missing/unused parameter checks, and conversion to `FilterIr`
   in `searchkernel-ir` (`filter_expression.rs` and the shared search resolver).
4. If the portable `FilterClause` shape changes, update `api/searchkernel.fbs`
   and regenerate the applicable FlatBuffers artifacts. Do not add client-side
   semantic parsers.
5. Add parser boundary/complexity tests plus schema-aware binder tests with
   precise structured errors and clause-local binding behavior.
6. Prove textual and structured filters resolve to equivalent `FilterIr`.
7. Change adapter/native lowering only if the resolved `FilterIr` capability
   itself changed; a pure syntax addition should normally stop before Vespa.
8. Update the capability ledger fields independently and regenerate the views.

Verify `searchkernel-filter`, `searchkernel-ir` filter/resolver tests, structured
vs textual equivalence, and a real Vespa lane only when execution semantics
changed.

## Recipe: add Python-only ergonomic sugar

Use this when the canonical portable model and shared semantics already expose
the capability.

1. Confirm the generated Python FlatBuffers object model already contains the
   required field or union. If it does not, this is a portable-schema change,
   not Python-only sugar.
2. Change only the thin facade, normally under
   `clients/python/searchkernel/ergonomic.py` or `client.py`.
3. Build the existing generated object model; do not add a parallel DTO or
   handwritten FlatBuffer codec.
4. Delegate defaults, normalization, validation, and errors to the canonical
   request/IR path.
5. Add Python unit tests for construction, packing, and the user-facing helper,
   plus an end-to-end test when the helper changes execution behavior.
6. Update only the ergonomic stage in `docs/capabilities.json`; do not mark
   semantic or executable support based on facade tests. Regenerate its views
   with `python3 tools/check-capabilities.py --write`.

Verify Python tests and, when applicable, the shared protocol/conformance suite.

## Recipe: change the native ABI

Use this only after proving that the existing Search Kernel-owned ABI cannot
express an already-verified Vespa capability.

1. Inspect the pinned Vespa source and tests and record a source trace for the
   capability. Do not modify `upstream/vespa/`.
2. Choose an additive function, typed setter/accessor, or opaque handle. Keep
   public arguments to fixed-width primitives, primitive arrays, byte ranges,
   and opaque handles; never expose Vespa or C++ object layout.
3. Define ownership, borrow lifetime, nullability, lengths, thread safety,
   error ownership, and exception behavior before editing code.
4. Update the public header, implementation, export/version map, and native
   adapter tests under `core/native/vespa/`.
5. Rebuild generated/prebuilt SDK output only through the repository tooling;
   it is written under the active environment's target root and must never be
   hand-edited. See [native-build-provenance.md](native-build-provenance.md).
6. Update the raw declarations in `searchkernel-vespa-bindings` and the safe
   `searchkernel-vespa-engine` wrapper. Direct FFI `unsafe` is allowed inside
   that engine boundary (the current wrapper uses it extensively); it must not
   leak into `searchkernel-ir`, the IR adapter, or public facade code. Keep the
   public wrapper safe and make ownership/lifetime invariants explicit. C++
   exceptions must be translated inside the C++ adapter rather than unwind
   across the C ABI.
7. Run `tools/check-vespa-bindings.sh` and add an executable real-native verifier,
   not just a symbol or source check.
8. Run the public-header ABI surface check and ABI comparison against the
   release-provided intentional baseline. `core/native/ABI.md` defines the
   baseline bundle and `tools/check-native-abi.sh` invocation. Do not create or
   refresh a baseline merely to make an ABI diff pass; if the release baseline
   is unavailable, report that comparison as not runnable. An incompatible
   change requires explicit review, an ABI version bump, and a new baseline.
9. Add focused ownership/lifetime, exception/error, concurrency, and regression
   tests, then run the affected cumulative Vespa verifier.
10. Update the applicable `docs/capabilities.json` support fields and evidence,
    then regenerate its views with `python3 tools/check-capabilities.py --write`.

At minimum, review `core/native/ABI.md`, `core/native/AGENTS.md`, the relevant
native adapter source, and the bindings crate before making this change. For an
ABI change, the normal checks include:

```bash
tools/check-vespa-bindings.sh
python3 tools/check_abi_surface.py core/native/vespa/searchkernel_vespa.h
tools/check-native-abi.sh \
  "$ABI_BASELINE/libsearchkernel_vespa.so" \
  "$BUILD/libsearchkernel_vespa.so" \
  "$ARTIFACT/searchkernel-vespa-engine.abi.xml"
```

## Completion checklist

Before declaring a capability complete:

- The contract and pinned-engine authority are identified.
- Portable semantics have one implementation in `searchkernel-ir`.
- Generated artifacts were regenerated only with pinned tooling.
- Positive, negative, malformed, and unsupported behavior is tested where
  applicable.
- The real backend/native path is exercised when the capability is executable.
- Rust and language helpers are clearly separated from portable semantics.
- Native ownership and ABI compatibility are documented for boundary changes.
- `docs/capabilities.json` records each verified support field independently and
  any intentional gap; generated views pass
  `python3 tools/check-capabilities.py --check`.
- Focused checks pass, followed by broader checks when the change crosses crate,
  API, IR, native, or client boundaries.

Useful verification entry points include:

```bash
# FlatBuffers schema changes
bash tools/generate-flatbuffers-bfbs.sh
tools/generate-searchkernel-protocol.sh
tools/generate-python-flatbuffers.sh   # when Python consumes the changed schema
bash tools/check-flatbuffers-bfbs.sh

# Frozen portable conformance
reference/api-conformance/verify.sh
# Only when testing a concrete conformance adapter:
reference/api-conformance/verify-adapter.sh /path/to/adapter

# Rust
cargo fmt --all --check
cargo clippy -p <affected-crate> --all-targets --all-features -- -D warnings
cargo test -p <affected-crate> --all-targets --all-features

# Capability ledger
python3 tools/check-capabilities.py --check

# Native ABI changes only
tools/check-vespa-bindings.sh
python3 tools/check_abi_surface.py core/native/vespa/searchkernel_vespa.h
```

For native or product-path changes, choose the exact applicable lane from the
authoritative map in `reference/vespa/README.md` (cumulative native baseline,
I-series product gate, or the applicable specialized lane). Expand to workspace checks when the change crosses shared infrastructure.
Do not substitute a nearby historical verifier merely because its name sounds
related.
