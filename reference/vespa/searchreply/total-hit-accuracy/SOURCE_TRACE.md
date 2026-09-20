# Total-hit accuracy source trace

Pinned Vespa revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The pinned `search::engine::SearchReply` in
`upstream/vespa/searchlib/src/vespa/searchlib/engine/searchreply.h` stores
`totalHitCount` and `Coverage`, but has no total-hit accuracy or relation bit.
`Coverage` only reports `covered`, `active`, `targetActive`, and the independent
`MATCH_PHASE`, `TIMEOUT`, and `ANN_TIMEOUT` degradation reasons.

The pinned result path is:

```text
upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/match_thread.cpp
  fillPartialResult(..., totalHits, ...)
upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/result_processor.cpp
  ResultProcessor::makeReply -> r.totalHitCount = result.totalHits()
upstream/vespa/searchlib/src/vespa/searchlib/engine/proto_converter.cpp
  search_reply_to_proto -> total_hit_count plus coverage/degradation fields
```

The verifier traces the known non-exhaustive cases before deciding the native
mapping:

| case | pinned source evidence | implication |
| --- | --- | --- |
| exhaustive ordinary query | `match_thread.cpp` fills the count; `result_processor.cpp` copies it, but no accuracy bit is added | the count may be exact, but the ABI does not prove it |
| weakAnd / WAND | `upstream/vespa/searchlib/src/vespa/searchlib/queryeval/wand/weak_and_search.cpp` seeks by score threshold and target-hit policy | a reported count has no exactness relation in `SearchReply` |
| nearest-neighbor | `upstream/vespa/searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.cpp` selects approximate index top-k and records ANN timeout statistics | coverage/ANN timeout metadata is not an exact-count guarantee |
| sorting with degrading optimization | `upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/matcher.cpp` and `match_phase_limiter.h` allow a limiter for sorted matching | the count is not made exact by the sort result path |
| match-phase degradation | `matcher.cpp:updateCoverage` sets `degradeMatchPhase()` when `limiter.was_limited()` | the result remains valid, but no exact-count signal appears |

Therefore the low-level adapter exposes `TotalHitsRelation::Unknown` for the
current native result ABI. It does not infer exactness from coverage, and it
does not mislabel an unknown count as `LowerBound`.
