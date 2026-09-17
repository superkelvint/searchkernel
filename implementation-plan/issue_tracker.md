# Issue Tracker

### 1. Add a real C7 verifier and wire it into acceptance

**Status: Complete.**

**Root cause:** The Python/C7 implementation exists, but there is no frozen C7 acceptance harness under `reference/clients/c7/`, so regressions can pass the rest of the verifier suite unnoticed.

**Fixed when:** A C7 contract/verifier exists, exercises the Python NativeTransport path, and is run by the normal acceptance flow.

### 2. Make C7 response testing exact

**Status: Complete.**

**Root cause:** Current C7 response checks mostly verify that decoded values are present/non-empty, so incorrect IDs, counts, scores, fields, facets, highlights, or errors could still pass.

**Fixed when:** Frozen response fixtures are replayed through the existing `c7-script` machinery and the decoded Python meaning is compared exactly against expected results.

### 3. Remove reopen-time portable → resolved equality dependency on current defaults

**Status: Complete.**

**Root cause:** Reopen re-resolves the persisted portable definition using today's C2 defaults and compares it to the persisted resolved definition. A future default change can therefore make an otherwise compatible index fail to reopen.

**Fixed when:** The persisted resolved definition is authoritative for reopen semantics; the original portable definition is retained only for provenance/inspection.

### 4. Fix post-rename create ambiguity around parent-directory fsync

**Status: Complete.**

**Root cause:** If the final directory rename succeeds but the subsequent parent-directory `fsync` fails, `Index::create()` returns an error even though the index may already exist at the final path.

**Fixed when:** Post-publication failures have explicit semantics so callers cannot receive an ordinary create failure for an index that has already been committed/published.

### 5. Fix the C5 describe lifecycle lease issue

**Status: Complete.**

**Root cause:** `describe` releases its backend-session operation lease before the actual describe work is complete, so `close()` can finish while an already-admitted describe is still running.

**Fixed when:** The describe operation executes entirely inside the session lease, matching the lifecycle behavior of other operations.

### 6. Restore C6 to fully green

**Status: Complete.**

**Root cause:** Generated metadata still expects an outdated frozen corpus count (`77` requests), while the current corpus contains `75` requests and `67` responses.

**Fixed when:** The C6 generated metadata and frozen corpus agree and the complete C6 verifier passes with no known-red exception.

### 7. Add timeout to the frozen C2 corpus

**Status: Complete.**

**Root cause:** Timeout was added across C1/C2/C4/M/native without first freezing its semantic behavior in the C2 conformance corpus, which allowed an adapter omission to survive.

**Fixed when:** C2 includes golden timeout cases covering absent/default behavior, normal values, boundaries, inspection, and relevant invalid inputs.

### 8. Make public result/diagnostic structs evolution-safe

**Status: Complete.**

**Root cause:** Public structs such as `SearchResult`, `Hit`, facet/highlight types, `TotalHits`, and `InspectedQuery` can be exhaustively constructed/destructured, making future fields source-breaking.

**Fixed when:** These public response/diagnostic types have an explicit evolution strategy, such as `#[non_exhaustive]` or private fields with accessors.

### 9. Replace native-error string matching with structured details

**Status: Complete.**

**Root cause:** The adapter currently detects some backend capability failures by matching human-readable error strings, making portable error behavior depend on wording.

**Fixed when:** M/native errors expose stable machine-readable detail codes/types and the adapter maps those instead of parsing prose.

### 10. Fix the Python null canonicalization helper

**Status: Complete.**

**Root cause:** `_value_from_json()` accepts canonical null as `{"null": null}`, while `_value_to_json(Value.null())` emits `{"null": {}}`, so the helpers are not inverses.

**Fixed when:** Python null encoding emits the canonical representation and round-trips through `_value_from_json()` unchanged.

## Implementation verification

All ten issues have been implemented and verified in the current tree:

- C7 replays all 67 frozen response fixtures exactly, including 58 errors and both introspection responses.
- C2 executes the frozen timeout cases through both search conversion and query inspection.
- C5, C6, and C7 acceptance verifiers pass.
- `cargo fmt --all --check`, workspace Clippy with warnings denied, and all workspace tests pass.
