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

The pinned setup has no stable, reasonable matcher query that produces a
positive `SearchReply::my_issues` message. The hardening-only native probe
therefore constructs a real pinned `SearchReply`, `Coverage`, and
`UniqueIssues` object, runs the same internal metadata-copy helper, and checks
all copied issue bytes, exact count, and invalid-index behavior through the
public result accessors. The limitation is only on obtaining positive issues
from a real matcher execution; issue preservation itself is executable.

The direct oracle and native probe emit the same canonical representation for
the normal and match-phase-degraded replies. Frozen expected output is checked
against both independently.
