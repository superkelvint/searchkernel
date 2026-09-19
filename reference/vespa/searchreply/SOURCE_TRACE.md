# SearchReply semantics source trace

Pinned Vespa revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

The source of truth is:

```text
upstream/vespa/searchlib/src/vespa/searchlib/engine/searchreply.h
```

`search::engine::Coverage` stores `covered`, `active`, `targetActive`, and a
degradation bitset. The pinned `DegradeReason` values are:

```text
MATCH_PHASE = 0x01
TIMEOUT     = 0x02
ANN_TIMEOUT = 0x04
```

The accessors used by the adapter are `getCovered()`, `getActive()`,
`getTargetActive()`, `wasDegradedByMatchPhase()`,
`wasDegradedByTimeout()`, and `was_degraded_by_ann_timeout()`.

The pinned reply also carries `UniqueIssues::UP my_issues`. The adapter copies
those messages into the result issue list; it does not merge them into the
native error buffer.

The Search Kernel mapping is implemented in:

```text
core/native/vespa/searchkernel_vespa.cpp
```

`EmbeddedEngine::search_local` copies the three coverage counters, maps the
three pinned `Coverage` predicates to the Search Kernel degradation constants,
and copies `my_issues` messages into the result. A valid degraded reply stays
successful; only an exception or other native operation failure becomes a
native error status.

The deterministic degraded-result setup is defined by the pinned match-phase
limiter and property name:

```text
upstream/vespa/searchcore/src/vespa/searchcore/proton/matching/match_phase_limiter.cpp
upstream/vespa/searchlib/src/vespa/searchlib/fef/indexproperties.cpp
vespa.matchphase.degradation.maxfiltercoverage
```

The pinned limiter has a minimum wanted-docs threshold of 128 in
`match_phase_limit_calculator.h`; the verifier therefore uses enough matching
documents to make the existing 0.9 setup actually exercise the limiter rather
than merely passing through an under-sized corpus.
