# C6 — BFBS-driven generation and retained JSON codec

This directory freezes the C6 acceptance contract.

## Normative source and BFBS compiler IR

The checked-in FlatBuffers schemas under `api/` are normative. The pinned FlatBuffers compiler is exactly `25.12.19`.

```text
api/searchkernel_request.fbs
api/searchkernel_response.fbs
       │
       │ flatc 25.12.19 -b --schema
       ▼
core/crates/searchkernel-json/bfbs/
       │
       ▼
searchkernel-bindgen
```

BFBS is the machine-readable compiler IR consumed by C6 generation. `searchkernel-bindgen` must not implement another `.fbs` parser.

The verifier proves this behaviorally: it creates schema material that does not exist in the repository, compiles it to BFBS, deletes the mutated `.fbs` files, then requires bindgen to generate the synthetic operation and field.

## `flatc` selection

Verifier discovery order:

1. `FLATC_PATH`;
2. `flatc` on `PATH`;
3. executable files named `flatc` under the repository `target/` tree.

The selected executable must report exactly `25.12.19`.

The verifier owns `.fbs → BFBS`. Bindgen receives BFBS only.

## Required bindgen CLI

The workspace contains `tools/searchkernel-bindgen/`, package name `searchkernel-bindgen`.

Frozen commands:

```text
searchkernel-bindgen generate \
  --bfbs-root <bfbs-directory> \
  --output-root <empty-output-directory>
```

Rules:

- `--output-root` mirrors generated repository-relative paths;
- generation must not modify the repository when `--output-root` is supplied;
- no `.fbs` or repository-root argument is supplied to bindgen;
- malformed/incomplete BFBS exits non-zero;
- unsupported or incomplete schema semantics fail explicitly rather than silently omitting output.

## Generated outputs and retained verifier artifacts

`GENERATED_PATHS.txt` freezes exactly the paths reproduced by
`searchkernel-bindgen`:

```text
clients/python/searchkernel/generated/models.py
reference/json-conformance/generated/schema-model.json
core/crates/searchkernel-json/src/generated.rs
```

The only generated artifacts retained by this contract are the three paths
listed above. The verifier also consumes two small, checked-in inventories
under `reference/json-conformance/expected/`:

```text
fixture-manifest.json
operation-map.json
```

`fixture-manifest.json` explicitly freezes the binary `.skrq` and `.skrs`
fixture names used by the retained codec corpus. No generated JSON mirror is
required.

## Operation coverage

`expected/operation-map.json` freezes:

```text
insert        InsertRequest       → InsertResult
upsert        UpsertRequest       → UpsertResult
patch         PatchRequest        → PatchResult
delete        DeleteRequest       → DeleteResult
get           GetRequest          → GetResult
search        SearchRequest       → SearchResult
analyze       AnalyzeRequest      → AnalyzeResult
describe      DescribeRequest     → DescribeResult
inspect_query InspectQueryRequest → InspectQueryResult
```

`Error` is the common portable error response.

These surfaces remain complete together:

```text
RequestPayload / ResponsePayload
operation map
generated Python BFBS domain models
retained JSON envelope codec
shared dispatcher
```

## Canonical Search Kernel JSON

Official FlatBuffers JSON is not public Search Kernel JSON.

Request:

```json
{"protocol_version":2,"request":{"search":{"query":"foo"}}}
```

Response:

```json
{"protocol_version":2,"response":{"search":{}}}
```

Portable error uses the one-key `error` response variant.

### Symbolic unions

Every FlatBuffers union is a symbolic one-key object, for example:

```json
{"string":"x"}
{"term_query":{"field":"title","value":{"string":"rust"}}}
```

Zero, multiple, or unknown variant keys are rejected. Raw wire discriminator fields such as `payload_type`, `kind_type`, `expr_type`, `policy_type`, `operation_type`, and `target_type` are forbidden in public JSON.

### Scalars

```text
bool                       JSON boolean
int32/uint32/bounded count JSON number
finite float32/float64     JSON number
int64                      canonical decimal JSON string
uint64                     canonical decimal JSON string
bytes                      base64 JSON string
```

64-bit integer strings are base 10, have no leading `+`, no unnecessary leading zero, no exponent/decimal point, reject `-0`, and enforce full ranges. Finite `float64` values use JSON numbers. Generic `Value` may represent NaN or infinities only through the symbolic `non_finite` / `NonFiniteFloatValue` representation (`nan`, `positive_infinity`, or `negative_infinity`). Raw JSON NaN/Infinity tokens are invalid, and tensor cells, timing, coverage, and other finite-only fields reject non-finite values.

### Presence and strictness

Preserve absent vs explicit false/zero/empty string/empty collection. JSON `null` is accepted only for semantic null.

Reject unknown fields, duplicate fields, unknown enum names, invalid union forms, out-of-range or non-canonical integer strings, malformed base64, invalid null, and raw official-FlatBuffers JSON.

## Generated metadata

`schema-model.json` is the generated BFBS schema inventory. The operation map
and binary fixture manifest are frozen acceptance inputs, not generated copies.
Canonical JSON behavior is verified directly by the retained codec contract.

## Deterministic generation

The verifier runs bindgen twice in clean output directories. Trees must be byte-for-byte identical and match checked-in generated outputs. No timestamps, absolute paths, random ordering, or host-specific data are permitted.

## BFBS-only mutation proof

The verifier adds temporary:

```text
VerifierProbeRequest
VerifierProbeResult
SearchRequest.verifier_probe_field
```

It compiles the temporary schemas to BFBS, deletes the `.fbs` sources, copies the bindgen executable into an isolated temp directory, and generates using only BFBS.

Required output includes operation metadata for `verifier_probe`, the generated
Python domain model, and schema metadata containing `verifier_probe_field`.

The verifier also appends `ErrorCode.VerifierSynthetic = 11` in a separate
BFBS-only mutation. Generation must either emit the variant and its complete
JSON conversion mappings or fail non-zero with a diagnostic naming the enum and
variant. Successful generation that silently omits the mapping is rejected.

## Fail-closed mutation proof

The verifier creates `VerifierOrphanRequest` with no result pair. Bindgen must fail non-zero and identify the orphan in its diagnostic. Malformed BFBS must also fail non-zero.

## Existing semantic authority

`reference/api-conformance` remains authoritative for FlatBuffers → `*_to_ir` → Search Kernel IR. Existing native-vs-JSON convergence tests remain required and must compare identical recorded IR.

## Forbidden shortcuts

Forbidden:

- reparsing `.fbs` inside bindgen;
- hardcoded current operation behavior;
- handwritten per-message JSON mapping;
- JSON → IR direct conversion;
- a second JSON semantic dispatcher;
- public official FlatBuffers JSON;
- silently skipped schema constructs;
- generation succeeding while omitting an operation;
- changing semantic corpus expectations merely to make C6 pass.
