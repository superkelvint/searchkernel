# SearchReply semantics contract

This verifier freezes the generic SearchReply result boundary independently of
the coverage-only tooling. It distinguishes three outcomes:

1. a normal usable result;
2. a usable result degraded by Vespa's match phase; and
3. a genuine native failure with no result.

For a normal result, coverage `active`, `covered`, and `targetActive` are
preserved, degradation flags are zero, and the result remains readable.

For a degraded result, the query uses the pinned Vespa match-phase limiter via
the real rank property
`vespa.matchphase.degradation.maxfiltercoverage=0.9`, together with the
limiter's real `qty` attribute and `maxhits` controls. The probe inserts a
corpus larger than the pinned limiter's 128-document lower bound, using the
same equality query, ascending attribute sort, and 0.9 max-filter-coverage
setup as the existing native coverage test. The verifier requires successful
status, a non-null readable result, populated coverage, and
`SK_DEGRADATION_MATCH_PHASE`. It does not manufacture Search Kernel flags.

The hard-failure case is a genuine invalid native search request (sorting on
the indexed, non-attribute `title` field). The status is non-OK, the result
output is cleared to null, and the diagnostic buffer is populated.
Degradation is therefore metadata on a valid result, not an error translation.

The pinned setup has no stable, reasonable query that produces a positive
`SearchReply::my_issues` message. This verifier consequently freezes the issue
accessor bounds/error behavior on results with zero issues, while leaving
future positive issue preservation additive.
