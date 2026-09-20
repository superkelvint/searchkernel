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

These stages are cumulative, but they are not a reason to add speculative
layers. First look for an existing type, IR variant, Vespa primitive, native
handle, or generated binding that already expresses the capability.

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

Keep the semantic implementation in `searchkernel-ir`. Rust and non-Rust entry
points must converge on the same meaning. Do not edit generated protocol/client
output by hand, patch `upstream/vespa/`, or add a native ABI function until the
Vespa capability and boundary need are demonstrated.

## Recipe: add a portable query node

Use this when a query expression must be expressible through the canonical
portable API.

1. Define the node, payload, discriminator, defaults, and malformed-input
   behavior in `api/searchkernel.fbs`.
2. Regenerate the committed BFBS and protocol bindings with the repository
   tooling. Regenerate language models only for supported SDKs that consume the
   changed schema.
3. Add FlatBuffers-to-IR conversion in
   `core/crates/searchkernel-ir/src/api_to_ir/query.rs`.
4. Add the owned representation in
   `core/crates/searchkernel-ir/src/model/query.rs`.
5. Add IR-to-FlatBuffers conversion in
   `core/crates/searchkernel-ir/src/ir_to_api/query.rs` when the node can appear
   in a response, snapshot, or round trip.
6. Add positive, malformed, unsupported, and nesting cases to the portable
   conformance suite. Assert the canonical error for invalid unions and
   capability mismatches.
7. Lower the typed query through
   `core/crates/searchkernel-ir-adapter/src/search/typed_query.rs` and its
   focused submodules.
8. Inspect the Vespa query model and pinned source. Extend
   `core/crates/searchkernel-vespa-engine/src/query/` only if the existing
   query primitive cannot represent the IR node.
9. Add or change the native ABI only when the engine wrapper lacks the required
   primitive. Follow the native ABI recipe below.
10. Add ergonomic Rust and Python helpers independently; generated low-level
    clients receive the schema capability without requiring facade sugar.
11. Update `docs/capabilities.json`, regenerate its views with
    `python3 tools/check-capabilities.py --write`, and record the highest
    verified stage.

Verify the shared API conformance suite, adapter tests, query lowering tests,
and the applicable real-Vespa lane. Do not accept a Rust-only query mock as
evidence of executable support.

## Recipe: add a field type

Use this when schema definitions, document values, or field capabilities need a
new type.

1. Define the portable field/value shape and optional-field semantics in
   `api/searchkernel.fbs`.
2. Regenerate BFBS and all required generated bindings.
3. Update schema and value conversion in the relevant
   `searchkernel-ir/src/api_to_ir/` modules.
4. Add the owned field/value representation under
   `searchkernel-ir/src/model/`, including capability validation and
   deterministic defaults.
5. Add response conversion under `searchkernel-ir/src/ir_to_api/` if the type
   can be returned, grouped, or exposed in schema results.
6. Trace every operation that consumes the type: document creation, updates,
   search/filter/sort/grouping, summaries, and result decoding. Do not claim a
   field type is supported merely because it parses.
7. Lower it through `searchkernel-ir-adapter` and the Vespa schema/document
   adapter. Inspect the pinned Vespa implementation and tests for the actual
   storage, indexing, attribute, and summary behavior.
8. Add positive and negative schema conformance cases, including invalid field
   capability combinations, empty/default values, round trips, and persistence
   reopen where applicable.
9. Add Rust and SDK helpers only after the portable and semantic paths are clear.
10. Update `docs/capabilities.json` by operation and stage, then regenerate its
    views with `python3 tools/check-capabilities.py --write`.

Verify API conformance, IR conversion tests, adapter model-mutation tests, and
the real Vespa schema/document lane required by the field capability.

## Recipe: add an update operator

1. Add the operator and its payload to the canonical update union in
   `api/searchkernel.fbs`.
2. Regenerate BFBS and generated protocol/client bindings.
3. Convert it in `searchkernel-ir/src/api_to_ir/document.rs` and add the owned
   operation to the document/update model.
4. Define malformed input, field-type legality, missing-field, and unsupported
   operator errors in the shared semantic path.
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
9. Update `docs/capabilities.json` with supported field types and stages, then
   regenerate its views with `python3 tools/check-capabilities.py --write`.

Verify focused IR, adapter, native CRUD, and cumulative update/persistence
acceptance tests.

## Recipe: add a grouping construct

1. Add the lossless grouping operation, expression, or value to
   `api/searchkernel.fbs`. Preserve structural distinctions that affect Vespa
   grouping semantics; do not encode raw grouping syntax as an untyped field.
2. Regenerate BFBS and generated bindings.
3. Add conversion in `api_to_ir/grouping.rs`, the owned form in
   `model/grouping.rs`, and response conversion in `ir_to_api/grouping.rs` or
   `ir_to_api/value.rs` as appropriate.
4. Add semantic validation for nesting, aliases, expression types, ordering,
   limits, and unsupported constructs.
5. Lower through `searchkernel-ir-adapter/src/search/grouping.rs` and inspect
   the pinned Vespa grouping behavior before extending the engine adapter.
6. Add conformance cases for a minimal valid construct, nested composition,
   malformed unions, invalid operands, and canonical result shape.
7. Add Rust/Python facade helpers independently. Keep grouping semantics in IR,
   not in the facade.
8. Add or update a real Vespa grouping verifier and source trace when execution
   changes.
9. Update `docs/capabilities.json` and regenerate its views with
   `python3 tools/check-capabilities.py --write`.

Verify portable conformance, adapter grouping tests, result decoding, and the
applicable grouping/YQL or cumulative Vespa verifier.

## Recipe: add a result value

1. Define the response value or result metadata in `api/searchkernel.fbs`.
   Change `api/searchkernel_response.fbs` only when the response envelope or
   payload union itself changes. Decide whether the value is a scalar, list,
   structured value, union, or opaque payload.
2. Regenerate BFBS and generated bindings.
3. Add the owned result/value type under `searchkernel-ir/src/model/result.rs`
   or `model/value.rs`.
4. Decode the backend/native value into IR in the adapter and convert it back
   through `searchkernel-ir/src/ir_to_api/result.rs` or `value.rs`.
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
2. Regenerate BFBS and generated bindings.
3. Resolve and validate it in `api_to_ir/ranking.rs`; add the owned setting to
   `model/ranking.rs`.
4. Convert it back through `ir_to_api/ranking.rs` if ranking appears in a
   response, snapshot, or canonical definition.
5. Lower it through the adapter’s search/ranking path and inspect the pinned
   Vespa ranking/query-property behavior. Do not implement ranking semantics in
   Rust.
6. Add valid, default, conflicting, malformed, and unsupported setting cases
   to the conformance suite.
7. Add the direct Rust builder and non-Rust ergonomic helper only as syntax
   sugar over the same IR setting.
8. Add a real Vespa source trace and verifier when execution or score behavior
   changes.
9. Update `docs/capabilities.json` and regenerate its views with
   `python3 tools/check-capabilities.py --write`.

Verify ranking IR tests, adapter lowering, portable goldens, and the applicable
ranking/match-feature Vespa verifier.

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
   never hand-edit `core/native/sdk/`.
6. Update raw Rust bindings and the safe Vespa-engine wrapper. Keep `unsafe` in
   the bindings layer and prevent Rust panics from crossing into C++.
7. Add an executable real-native verifier, not just a symbol or source check.
8. Run the ABI surface check and ABI comparison against the intentional
   baseline. An incompatible change requires explicit review, an ABI version
   bump, and a new baseline.
9. Add focused ownership/lifetime, exception/error, concurrency, and regression
   tests, then run the affected cumulative Vespa verifier.
10. Update `docs/capabilities.json` with the native boundary and verified stage,
    then regenerate its views with `python3 tools/check-capabilities.py --write`.

At minimum, review `core/native/ABI.md`, `core/native/AGENTS.md`, the relevant
native adapter source, and the bindings crate before making this change.

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
- `docs/capabilities.json` records the verified stage and any intentional gap;
  generated views pass `python3 tools/check-capabilities.py --check`.
- Focused checks pass, followed by broader checks when the change crosses crate,
  API, IR, native, or client boundaries.

Useful verification entry points include:

```bash
tools/generate-flatbuffers-bfbs.sh
tools/generate-searchkernel-protocol.sh
reference/api-conformance/verify.sh
reference/api-conformance/verify-adapter.sh
cargo fmt --all --check
cargo clippy -p <affected-crate> --all-targets --all-features -- -D warnings
cargo test -p <affected-crate> --all-targets --all-features
python3 tools/check_abi_surface.py core/native/vespa/searchkernel_vespa.h
```

For native or product-path changes, also run the exact applicable command from
`reference/vespa/README.md`, and expand to workspace checks when the change
crosses shared infrastructure.
