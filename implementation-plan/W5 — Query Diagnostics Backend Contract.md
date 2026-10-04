# W5 — Query Diagnostics Backend Contract

**Status:** Proposed technical contract  
**Product:** Search Kernel / searchkerneld / Searchkernel Web  
**Companion UX spec:** `W5 — Search Understanding and Query Debugger Specification.md`

---

# 1. Purpose

This document defines the backend and evidence contract required by the W5
Search Understanding and Query Debugger UX.

The core rule is:

> Search Kernel owns diagnostic semantics. Searchkernel Web presents findings and
> evidence; it does not interpret Vespa query/ranking semantics independently.

The ordinary Search Kernel search API remains the production semantic contract.
Deep Vespa diagnostic payloads must not be added indiscriminately to every normal
search response.

---

# 2. Diagnostic operation

Introduce an explicit generation-bound diagnostic operation.

Conceptually:

```text
QueryDiagnosticsRequest
  target
  generation
  canonical_query
  intent
  requested_evidence
  selected_hits?
  expected_document?
  experiment?
  limits?
```

The exact FlatBuffers/API naming is implementation work.

The operation must use the same canonical target/incarnation semantics as normal
search.

---

# 3. User-problem intents

The public contract should be typed around product intent rather than raw Vespa
parameters.

Candidate intents:

```text
EXPLAIN_HIT
ZERO_RESULTS
MISSING_DOCUMENT
RANKING_COMPARISON
OVERMATCHING
QUERY_INTERPRETATION
VECTOR_RETRIEVAL
PERFORMANCE
DEEP_ENGINE
EXPERIMENT
```

These intents may internally map to different Vespa query parameters and
diagnostic controls.

The browser must not need to construct arbitrary Vespa trace parameters for the
normal product workflows.

---

# 4. Evidence model

Conceptually:

```text
QueryDiagnostics
  request_identity
  target_identity
  source_run
  rerun_metadata?
  findings[]
  evidence[]
  effective_query?
  retrieval?
  ranking?
  timing?
  hit_evidence[]
  experiment_comparison?
  warnings[]
  raw_engine_diagnostics?
```

Only concepts Search Kernel is willing to support durably should be normalized.

Raw Vespa data remains engine-specific.

---

# 5. Findings

A finding is a deterministic backend-owned statement derived from explicit
diagnostic evidence.

Examples:

- a specific filter variant is sufficient to change 0 results to N results;
- an expected document is returned without filter X but not with filter X;
- the document was not present in the observed retrieval candidate evidence;
- exact NN returned a document that approximate NN did not;
- first-phase profiling consumed more measured time than second-phase profiling.

Findings must reference the evidence items that support them.

Do not produce speculative natural-language causes.

---

# 6. Provenance

Every evidence item carries one primary provenance:

- `PROFILE` — intentionally configured schema/rank-profile feature;
- `ENGINE` — reported by Search Kernel/Vespa execution;
- `CALCULATED` — deterministic arithmetic/comparison over explicit evidence.

Raw Vespa evidence additionally carries engine version/provenance metadata.

Avoid multiplying UI-facing provenance taxonomies.

---

# 7. Original run versus rerun

Diagnostics frequently require rerunning a query.

Each diagnostic response must make clear whether evidence comes from:

- the original search response;
- a diagnostic rerun;
- an experiment;
- a counterfactual forced-document evaluation.

Conceptually:

```text
RerunMetadata
  kind
  original_request_hash
  diagnostic_request_hash
  original_target_generation
  diagnostic_target_generation
  result_overlap?
  result_changed
```

If the result set differs, surface sufficient metadata for the UI to warn the
user.

Never silently attach rerun evidence to an earlier hit as if it came from the
same execution.

---

# 8. Query interpretation

The backend may expose:

- canonical Search Kernel query;
- engine request/YQL;
- transformed query tree;
- query blueprint;
- iterator tree;
- backend-generated friendly query rendering.

Friendly rendering must be generated from backend-owned query semantics.

The frontend must not parse arbitrary Vespa/YQL syntax to produce semantic
explanations.

---

# 9. Ranking evidence

Support, where available:

- rank profile identity;
- first-phase score;
- first-phase rank;
- first-phase maximum;
- second-phase participation/score;
- global-phase participation/final relevance;
- configured match features;
- configured summary features;
- query features;
- named rank-profile function values;
- raw rank-feature dump for deep diagnostics.

Feature values may be scalar or tensor.

The schema must not assume numeric double-only features.

---

# 10. No-named-signals baseline

EXPLAIN_HIT must return useful structural evidence even when no named profile
features exist.

At minimum, where supported:

- returned position;
- final relevance;
- rank profile;
- which ranking phases are configured;
- first-phase rank/score;
- whether the hit entered second-phase reranking;
- second/global phase score where available.

The response should explicitly indicate that semantic explanation signals are not
configured.

---

# 11. Result-set context

To contextualize named signals without inventing cross-scale importance, the
backend or UI may calculate within-returned-set statistics:

- min/max among returned hits;
- ordinal rank among returned hits;
- equal-value/tie handling.

The comparison set must be explicit.

Do not label such values as corpus or matched-set percentiles.

---

# 12. Zero-results diagnostics

The backend owns construction of controlled diagnostic variants.

Given a canonical query, it may produce safe variants such as:

- remove one filter;
- remove one positive term/clause;
- relax one supported constraint.

Each variant returns:

```text
DiagnosticVariant
  description
  canonical_request
  changed_component
  result_count
  result_sample?
```

Do not generate variants that change semantics in ways the backend cannot
describe faithfully.

Top-k/non-monotonic operators require engine-aware handling.

A finding such as "removing filter X is sufficient to produce results" is allowed
because it is directly supported by the variant execution.

---

# 13. Missing-document diagnostics

Expected-document selection may resolve from:

- canonical document ID;
- document lookup/search result;
- unique configured field value.

The backend should expose stage evidence when possible:

```text
DocumentDiagnostic
  exists
  explicit_filter_evidence?
  retrieval_candidate_evidence?
  first_phase_evidence?
  second_phase_evidence?
  returned
```

Each stage is tri-state or richer:

- observed yes;
- observed no;
- unavailable.

Do not convert "not returned" into "did not match."

---

# 14. Counterfactual diagnostics

A backend may support forced/counterfactual evaluation of ranking features for a
known document.

Such responses must carry:

```text
execution_kind = COUNTERFACTUAL
```

and must never be merged with original-execution evidence.

The contract must make it impossible for clients to mistake counterfactual score
evidence for proof that the document entered the original candidate set.

---

# 15. Ranking comparison

For two hits from the same run, expose aligned evidence keys and phase data.

The backend/UI may calculate:

- within-returned-set feature rank;
- equality/ties;
- final rank movement between phases where observable.

Do not calculate a generic "importance" from raw numeric differences across
heterogeneous feature scales.

---

# 16. Matched-term evidence

When Vespa/Search Kernel can authoritatively expose term/field match evidence,
normalize it.

Conceptually:

```text
MatchedTerm
  term
  field
  source
  optional_offsets?
```

Do not reconstruct matching by searching returned text in the frontend.

---

# 17. Vector/hybrid diagnostics

For nearest-neighbor retrieval expose, where supported:

- tensor field;
- query tensor metadata;
- distance metric;
- approximate flag;
- target-hit settings;
- HNSW exploration settings;
- distance/closeness;
- candidate counts;
- filter interaction evidence.

Support an explicit exact-comparison diagnostic:

```text
VectorComparison
  approximate_run
  exact_run
  overlap_at_k[]
  rank_movements[]
  latency
```

Only expose "recall@k" when the exact run is a valid reference for that metric.

Exact comparison is a diagnostic rerun and may be expensive.

---

# 18. Performance diagnostics

Support progressive evidence acquisition:

1. ordinary timing;
2. trace timestamps;
3. matching profiling;
4. first-phase profiling;
5. second-phase profiling.

Relevant Vespa controls may include:

- `trace.level`;
- `trace.query`;
- `trace.timestamps`;
- `trace.explainLevel`;
- `trace.profileDepth`;
- `trace.profiling.matching.depth`;
- `trace.profiling.firstPhaseRanking.depth`;
- `trace.profiling.secondPhaseRanking.depth`.

These are implementation details of the Vespa adapter, not required browser
parameters.

Profiler output should preserve tree structure and self/total timing when Vespa
provides it.

---

# 19. Raw Vespa diagnostics

Raw engine data may include:

- request;
- response;
- trace;
- blueprint;
- iterator tree;
- rank-feature dump;
- matching profiler;
- ranking profilers.

Requirements:

- mark as engine-specific;
- attach Vespa/native version metadata;
- bound response size;
- do not promote unstable raw field names into stable Search Kernel semantics;
- preserve raw payload for expert inspection where practical.

---

# 20. Diagnostic cost and limits

The backend must bound expensive diagnostics.

Controls should include safe maxima for:

- trace level;
- profiler depth;
- number of hits with full features;
- tensor cells returned;
- raw diagnostic bytes;
- variant count;
- exact-NN comparison size;
- diagnostic timeout.

The public API may expose typed limits, but clients should not need arbitrary
engine knobs for common workflows.

Normal search must not automatically enable deep diagnostics.

---

# 21. Cancellation and mutation safety

Diagnostic reruns:

- never mutate index state;
- follow normal timeout/cancellation semantics;
- preserve target generation;
- must not report a cancelled request as authoritative complete evidence.

If the original search had mutation-adjacent state changes, the rerun metadata
must make result divergence visible.

---

# 22. Partial and unsupported evidence

Diagnostics may succeed partially.

The response must support per-section status:

- available;
- unsupported;
- timed out;
- truncated;
- unavailable for this query;
- failed.

Do not collapse partial evidence into a single generic error when useful evidence
was collected.

Unsupported engine/version capabilities must be distinguishable from transient
failure.

---

# 23. Experiment contract

The W5 experiment lane is a controlled diagnostic rerun, not persistent
configuration mutation.

Conceptually:

```text
QueryExperiment
  baseline_request
  change
  experiment_request
```

Supported changes may include:

- rank profile;
- filter;
- supported query input;
- vector target-hit settings;
- approximate/exact switch.

Response may contain:

- result overlap;
- entering/leaving hits;
- rank movement;
- timing comparison;
- named-signal comparison.

Every experiment is explicitly identified as a rerun.

---

# 24. Sharing/export

Support generation of a diagnostic bundle containing, subject to explicit user
choices:

- Search Kernel version;
- Vespa/native version;
- target/index identity;
- canonical request;
- normalized findings/evidence;
- rerun metadata;
- optional raw engine diagnostics.

The API should expose enough stable information for the UI to generate a
reproducible public-API request / "Copy as curl".

Credentials must not be included in exported commands/bundles automatically.

---

# 25. Verification

Contract tests must cover:

- canonical encoding/decoding;
- every supported diagnostic intent;
- stale-generation rejection;
- original-vs-rerun identity;
- result-divergence metadata;
- no-named-signals baseline;
- zero-result variants;
- missing-document tri-state stages;
- counterfactual labelling;
- scalar/tensor features;
- partial evidence;
- unsupported capability;
- timeout/truncation;
- raw engine isolation;
- experiment reruns.

Real-engine tests must cover at least:

- lexical profile with named functions;
- profile without named explanation functions;
- first + second phase;
- global phase when supported;
- zero-results elimination;
- missing result;
- approximate NN;
- exact-vs-approx comparison;
- matching profiling;
- first-phase profiling;
- second-phase profiling;
- restart/reopen;
- delete/recreate stale-generation rejection.

---

# 26. Vespa capability references

Implementation should track current Vespa behavior, especially:

- https://docs.vespa.ai/en/reference/api/query.html
- https://docs.vespa.ai/en/ranking/phased-ranking.html
- https://docs.vespa.ai/en/basics/ranking.html
- https://docs.vespa.ai/en/reference/ranking/rank-features.html
- https://docs.vespa.ai/en/reference/schemas/schemas.html
- https://docs.vespa.ai/en/reference/querying/yql.html
- https://docs.vespa.ai/en/querying/nearest-neighbor-search-guide.html

If actual embedded Vespa behavior differs, update this contract rather than
adding frontend heuristics.
