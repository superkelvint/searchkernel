# M8 — Local search request and results

## Goal

Complete the ordinary **local** Vespa search request/result path exposed by `searchkernel-vespa`.

M8 starts from the M7 typed query tree and adds the request/result capabilities normally surrounding matching:

```text
M7 Query
  ↓
field-aware lowering
  ↓
local Vespa SearchRequest
  ↓
DocumentDB::match
  ↓
local SearchReply
  ↓
optional local DocumentDB::getDocsums
  ↓
LocalSearchResult
```

M8 does not reproduce any distributed query dispatch or distributed fill/result-merge layer.

## Frozen API

The public Rust shape is frozen by:

```text
reference/vespa/api-contract/src/bin/m8.rs
```

M8 adds:

```text
Value
SortOrder
SortField
SummaryRequest
LocalSearchRequest
LocalHit
LocalSearchResult
Engine::search_local
```

This is a low-level Vespa-facing API, not the C-series product search API.

## Request semantics

`LocalSearchRequest::new(query)` has these defaults:

```text
rank_profile = "default"
limit        = 10
offset       = 0
sort         = []
summary      = SummaryRequest::None
```

Builder calls replace/append those values exactly as shown by the frozen API fixture. `rank_profile`, `limit`, `offset`, and `summary` replace the previous setting. Each `sort_by` appends one sort key in call order; M8 does not silently deduplicate sort keys.

### Query

The query is the M7 `Query` representation. All M7 field/type/linguistics rules remain in force.

Structured filters therefore execute inside the Matcher. M8 must not fetch a broader hit set and filter it in Rust.

### Limit

`limit` maps to the local Vespa request's maximum-hit count.

`limit = 0` is valid. It returns no regular hits but still reports the engine's total hit count where the engine provides it.

### Offset

M8 requires the pinned engine's native local offset capability.

`offset` maps directly to the local Vespa request. Search Kernel must not emulate offset by requesting `offset + limit` and dropping hits in Rust.

### Sorting

M8 v1 supports one or more sort keys over M6 **Integer attribute** fields.

```text
Ascending(field)
Descending(field)
```

Sort keys are applied in request order.

Requirements:

```text
field exists
field type == Integer
field has attribute=true
```

Unknown fields and non-attribute fields are deterministic errors. M8 does not
freeze rejection of attribute types that a later cumulative milestone may
support; those capability ceilings belong in the milestone that introduces
them, not in an earlier verifier that later milestones must pass.

M9 broadens the supported attribute types. M8 itself does not require
String/Float sort support.

The verifier avoids ambiguous sort ties unless a later sort key resolves them.

### Rank profile

`rank_profile` names an existing native rank profile and is sent to Vespa. M8 does not add rank-expression configuration; M10 owns that.

### Summaries / stored fields

M8 wraps the local docsum path.

```text
SummaryRequest::None
  no docsum request

SummaryRequest::Default
  local default summary class

SummaryRequest::Fields(Vec<String>)
  local default summary class + field list
```

`Fields` requirements:

```text
non-empty
no duplicates
all fields exist
all requested user fields have summary=true in M6 schema metadata
```

`documentid` is not included in the returned summary map because the public hit already exposes `document_id` separately. Vespa-internal/default summary entries such as `rankfeatures` and `summaryfeatures` are also not exposed by M8; M10 owns explicit feature inspection. `LocalHit.summary` contains only user-declared M6 fields with `summary=true` (or the explicitly requested subset).

The verifier includes a Text field with:

```text
index=true
summary=true
attribute=false
```

Returning that field proves the implementation actually wraps Vespa's local docsums/document-store path. It cannot be reconstructed from an attribute value.

## Result semantics

```text
LocalSearchResult {
    hits,
    total_hit_count,
}
```

### Hits

Hit order is the real local Vespa result order after native matching/sorting/pagination.

Rust must not re-sort hits.

### Document IDs

Each hit exposes the real Vespa document ID corresponding to the returned global/document hit.

### Scores

`LocalHit.score` is the real Vespa hit rank/metric from the local `SearchReply`.

The verifier compares scores by IEEE-754 `f64::to_bits()` representation.

Rust must not calculate BM25 or any other score.

### Total hit count

`total_hit_count` is the total-hit information supplied by the local engine.

It is **not**:

```text
hits.len()
limit
limit + offset
```

A request returning two hits from a five-hit match set must report five when the pinned engine reports five.

## Required native behavior

The implementation must ultimately use the pinned Vespa C++ local request/result path corresponding to:

```text
search::engine::SearchRequest.offset
search::engine::SearchRequest.maxhits
search::engine::SearchRequest.sortSpec
proton::DocumentDB::match
search::engine::SearchReply.hits
search::engine::SearchReply.totalHitCount
search::engine::Hit::metric
proton::DocumentDB::getDocsums
search::engine::DocsumRequest
```

Exact internal helper choices may follow the pinned revision. The semantics above are frozen.

## Required scenarios

The frozen M8 corpus contains:

```text
01 limit + offset + ascending integer sort + selected summary
02 descending integer sort
03 M7 structured integer range filter + sort + pagination
04 limit=0 with non-zero total hit count
05 offset past end with unchanged total hit count
06 BM25 text search with real scores + default summary
07 text field summary requested explicitly from non-attribute stored field
08 two-key sort that removes ties
```

The scenario composition is intentional:

- #01 fails if the implementation sorts only the already-paginated Rust hit list;
- #03 fails if M7 filtering is post-processed;
- #04/#05 fail if total hits are derived from returned hits;
- #06 proves score transport;
- #07 proves local docsums/document-store access.

## Required negative cases

```text
unknown sort field
sort on Text non-attribute field
unknown summary field
summary field with summary=false
empty explicit summary field list
duplicate explicit summary field
```

M7 query errors remain covered by M7 and need not be duplicated.

## Oracle

M8 behavior is frozen from a direct pinned Vespa C++ oracle.

The oracle must:

- instantiate the real local DocumentDB/PersistenceEngine path;
- use real `SearchRequest` fields and real `SearchReply` data;
- use real local docsums;
- receive M3 document/query analysis for text cases rather than implementing linguistics itself;
- emit canonical JSON using exact score bits;
- not link or call Search Kernel production code.

See `oracle/ORACLE_SPEC.md`.

## Forbidden shortcuts

Production must not:

```text
sort hits in Rust
slice/drop hits in Rust to emulate offset
compute total_hit_count from returned hits
compute scores in Rust
reconstruct non-attribute summaries from fixture/source data
read reference/vespa/m8 scenarios or expected data
invoke Java/Maven/JVM
shell out to an external Vespa process/service
import searchkernel-core
return fixture-specific output
```

A local C++ adapter that translates Search Kernel-owned request/result values into Vespa internal types is the intended implementation.

## Definition of done

M8 is complete when:

```text
M0-M7 remain green
frozen m8.rs compiles
all direct C++ M8 goldens reproduce exactly
Rust M8 output matches the same goldens
native limit/offset/sorting semantics are proven
M7 structured filtering composes with M8 request controls
real scores are exposed
engine total-hit information is exposed
local docsums return stored/summary fields
negative validation cases fail as frozen
no distributed dispatch/fill/merge implementation exists
no Rust post-processing substitute exists
verify-m8.sh passes
```
