# W5 — Search Understanding and Query Debugger Specification

**Status:** Proposed implementation specification  
**Product:** Searchkernel Web  
**Milestone:** W5, immediately after W4 index operations and diagnostics  
**Primary UI reference:** Algolia-style progressive disclosure  
**Engine depth:** Vespa query, retrieval, ranking, and profiling diagnostics  
**Architecture rule:** Search Kernel owns semantics; the frontend presents and navigates evidence

---

# 1. Purpose

W5 turns Searchkernel Web from a search workbench into a search debugger.

The product goal is not to expose every Vespa diagnostic control in the default UI. The goal is to let a user move progressively from:

> “Why does this result look wrong?”

to:

> “Show me the exact query tree, candidate-generation behavior, ranking features,
> execution blueprint, iterator tree, and profiling output.”

without forcing the first user to think like the second user.

The governing product principle remains:

> **Algolia for usability, Vespa for conceptual depth, Search Kernel for product
> vocabulary and semantics.**

W5 must therefore satisfy both of these requirements:

1. A developer who knows search concepts but not Vespa can diagnose common relevance
   and query problems through guided workflows.
2. A Vespa engineer can drill through every abstraction and inspect the authoritative
   engine evidence needed for serious debugging.

Every friendly explanation must have a “show me the evidence” path.

---

# 2. Product outcome

W5 is complete when Searchkernel Web can answer these questions from a real query:

- What query did Search Kernel actually send to the engine?
- How did Vespa transform and execute it?
- Why did this document match?
- Why is result A above result B?
- Why is an expected document missing?
- Where did a document fall out of the retrieval/ranking pipeline?
- Which ranking signals materially differ between two hits?
- Which ranking phase produced the effective score?
- Is approximate nearest-neighbor retrieval causing recall loss?
- Why is this query slow?
- Which search iterators or ranking features consume the time?
- What raw Vespa evidence supports the UI's explanation?

The normal search experience must remain simple and fast when none of those
questions are being asked.

---

# 3. Progressive-disclosure model

W5 uses five levels of disclosure.

## 3.1 Level 0 — Search

This is the ordinary Search Lab experience.

The user sees:

- query controls;
- selected rank profile;
- filters and sorting;
- returned hits;
- final relevance where meaningful;
- timing and coverage at the existing normal level.

The user does **not** see by default:

- Vespa trace levels;
- query blueprints;
- iterator trees;
- HNSW internals;
- complete rank-feature dumps;
- match/first-phase/second-phase profiling controls.

Each hit gains a low-friction affordance:

> **Why this result?**

The query gains:

> **Understand this search**

No diagnostic instrumentation should be enabled merely because the Search Lab is
open.

## 3.2 Level 1 — Explain

This is the default human-readable explanation layer.

For a selected hit, prefer a compact explanation built only from authoritative,
named evidence already exposed by the rank profile or Search Kernel.

Example:

```text
Why this ranked #1

Strong text relevance
  Text relevance signal: 3.91

Strong semantic similarity
  Semantic signal: 0.84

Popularity helped
  Popularity signal: 0.73

Ranking profile
  semantic-v2

[Show ranking details]
```

This layer should use human labels when the underlying signals are intentionally
named, for example rank-profile functions such as:

- `lexical_score`;
- `semantic_score`;
- `freshness_score`;
- `business_score`.

It must not reverse-engineer arbitrary Vespa ranking expressions and then invent
a semantic explanation.

If a profile does not expose meaningful explanation signals, say so explicitly:

> This ranking profile does not expose named explanation signals.

Then offer the technical evidence.

## 3.3 Level 2 — Troubleshoot

This is a guided diagnostic layer organized around user problems, not Vespa API
parameters.

Entry point:

> **What's wrong with this search?**

Initial guided intents:

- A result is ranked too high or too low.
- A result I expected is missing.
- Too many or irrelevant results are matching.
- Vector/hybrid search looks wrong.
- The query is slow.
- I want to understand the query itself.

The selected intent determines the diagnostic preset and what instrumentation is
requested.

The user should not need to know that a workflow uses
`trace.explainLevel`, `ranking.listFeatures`, or a profiling depth.

## 3.4 Level 3 — Search engineer

This level exposes the actual search pipeline explicitly.

The user can inspect:

```text
Query processing
    ↓
Retrieval / matching
    ↓
First-phase ranking
    ↓
Second-phase ranking
    ↓
Merge
    ↓
Global-phase ranking
    ↓
Fill / summary
```

Each stage can expose the relevant engine concepts and values.

At this level it is appropriate to show terms such as:

- query tree;
- `weakAnd`;
- WAND;
- `nearestNeighbor`;
- target hits;
- first phase;
- second phase;
- global phase;
- match features;
- summary features;
- query features;
- rank features;
- score-drop limits;
- rerank counts.

## 3.5 Level 4 — Vespa internals

This is the no-abstraction expert view.

It must provide direct access, where available, to:

- raw Vespa request;
- raw Vespa response;
- trace output;
- `trace.level`;
- `trace.explainLevel`;
- query blueprint;
- search iterator tree;
- `ranking.listFeatures` output;
- raw match features;
- raw summary features;
- matching profiler output;
- first-phase ranking profiler output;
- second-phase ranking profiler output;
- engine-specific diagnostic warnings.

Searchkernel Web may organize this data, but must not hide or reinterpret the raw
evidence in this view.

---

# 4. Diagnostic cost model

Progressive disclosure applies to query cost as well as visual complexity.

Define four conceptual execution modes:

| Mode | Intended use | Engine overhead |
| --- | --- | --- |
| Normal | ordinary Search Lab | minimal |
| Explain | explain returned hits using deliberately exposed features | low |
| Diagnose | guided troubleshooting with targeted instrumentation | medium |
| Deep diagnose | Vespa trace/explain/profiling/full feature inspection | high |

The exact backend API does not need to expose these literal names if a better
typed model emerges, but the cost boundaries must remain.

## 4.1 Normal

Normal search must not automatically request full rank features, trace plans, or
profiling.

## 4.2 Explain

Prefer already configured `match-features` / `summary-features` and named
rank-profile functions.

This is the intended inexpensive path for “Why this result?”

## 4.3 Diagnose

Rerun the query with the minimum additional instrumentation required by the
selected troubleshooting intent.

Examples:

- ranking comparison -> selected rank features and phase scores;
- missing result -> retrieval/query-plan diagnostics;
- slow query -> trace timestamps and targeted profiling;
- vector problem -> ANN/exact comparison and vector-specific settings.

## 4.4 Deep diagnose

Deep diagnostics may alter latency and produce large responses.

The UI must state that clearly:

> This reruns the query with expensive diagnostic instrumentation. Diagnostic
> latency is not representative of normal execution.

Deep diagnostics are opt-in.

---

# 5. Vespa execution model represented by the UI

The UI must reflect Vespa's actual execution stages rather than inventing a
generic “relevance score” model.

## 5.1 Query processing and transformation

Vespa query tracing can expose progressively deeper information about query
transformation and execution.

Relevant engine controls include:

- `trace.level`;
- `trace.query`;
- `trace.timestamps`;
- `trace.explainLevel`.

The debugger should be able to show a progression such as:

```text
User input
    ↓
Canonical Search Kernel query
    ↓
Vespa request / YQL
    ↓
Parsed / transformed query tree
    ↓
Dispatched query
```

The exact representation available at each point depends on the Search Kernel
adapter and Vespa diagnostics. The UI must label provenance rather than implying
that all stages are canonical Search Kernel objects.

## 5.2 Retrieval and matching

Retrieval must be shown separately from ranking.

Relevant cases include:

- term retrieval;
- filters;
- `weakAnd`;
- WAND;
- nearest-neighbor retrieval;
- combinations of lexical and vector retrieval.

The UI must not present `totalCount` or target-hit values as “all logically
matching documents” when the retrieval operator only exposes a bounded candidate
set.

The debugger should be able to show, where evidence exists:

```text
Corpus
  ↓
retrieval / matching
  ↓
candidates exposed to first phase
  ↓
first-phase ranking
  ↓
second-phase selection
  ↓
merge
  ↓
global phase
  ↓
returned hits
```

Counts must be labelled by their actual semantics.

## 5.3 First-phase ranking

Vespa evaluates the configured first-phase expression over documents exposed to
ranking by retrieval.

For a selected hit, expose where available:

- first-phase expression or named function;
- `firstPhase`;
- `firstPhaseRank`;
- `firstPhaseMax`;
- rank-score-drop behavior;
- component rank features used by the profile.

## 5.4 Second-phase ranking

When configured, expose:

- whether the hit entered the second-phase rerank set;
- configured total/per-node rerank bounds;
- `secondPhase`;
- relevant rank features/functions;
- score-drop behavior where configured.

Do not imply that every first-phase hit is second-phase ranked.

## 5.5 Global phase

When configured, expose global phase separately from content-node ranking.

Show:

- that the phase is global/container-side;
- rerank count;
- normalized/cross-hit features where relevant;
- final relevance after global reranking.

A first-phase or second-phase score must not be labelled “final score” when
global-phase changes the result ordering.

## 5.6 Fill / summary

Show summary/fill as a distinct phase when relevant to diagnostics.

This matters because `summary-features` are recalculated during fill for the
final selected hits, while `match-features` are computed in the match protocol.

The UI should preserve that distinction so users do not assume every displayed
feature value came from the same phase.

---

# 6. Ranking evidence model

## 6.1 Named functions are the preferred human explanation vocabulary

Vespa rank-profile functions provide the best bridge between arbitrary ranking
power and an understandable UI.

Example profile:

```text
function lexical_score() {
    expression: bm25(title) + bm25(body)
}

function semantic_score() {
    expression: closeness(field, embedding)
}

function freshness_score() {
    expression: ...
}
```

If these functions are exposed through match/summary features, the UI can present:

```text
Ranking signals

Lexical relevance       3.72
Semantic relevance      0.86
Freshness               0.54
```

W5 should encourage this pattern without requiring Search Kernel to understand
the meaning of arbitrary expression syntax.

## 6.2 Match features

Configured `match-features` are computed during the match protocol and returned
per hit.

Use them when the diagnostic needs values from the matching/ranking phase and the
profile intentionally exposes those values.

## 6.3 Summary features

Configured `summary-features` are recalculated during fill for the final hits.

Use them for inexpensive, result-focused explanations when their phase semantics
are appropriate.

The UI must label feature provenance if the distinction matters.

## 6.4 Full rank-feature dump

`ranking.listFeatures=true` can request the configured/default rank-feature
dump.

This belongs in Diagnose/Deep diagnose, not Normal.

The feature inspector must support both scalar and tensor feature values.

Do not design the UI under the assumption that every rank feature is a double.

---

# 7. Core user workflows

## 7.1 Why this result?

Entry point: each search hit.

Default output should answer:

- which rank profile ran;
- which named explanation signals are available;
- what those signals were for this hit;
- which ranking phases were configured;
- the final relevance value;
- whether deeper diagnostics are needed to explain more.

The compact explanation must link to:

> Show ranking details

and ultimately:

> Vespa internals

## 7.2 Why is A above B?

Allow selecting two hits from the same query.

Show side-by-side values for:

- final relevance;
- first-phase score/rank where available;
- second-phase score/participation;
- global-phase evidence where available;
- named ranking functions;
- selected match/summary features;
- query inputs shared by the run.

Example:

| Signal | Hit A | Hit B |
| --- | ---: | ---: |
| lexical_score | 5.21 | 4.10 |
| semantic_score | 0.71 | 0.84 |
| firstPhase | 5.78 | 4.77 |
| firstPhaseRank | 3 | 17 |
| secondPhase | 0.82 | 0.91 |
| final relevance | 0.89 | 0.84 |

The UI may highlight the largest numerical differences, but must not claim causal
importance beyond what the ranking expression/evidence establishes.

## 7.3 A result I expected is missing

The guided flow takes a known document identifier plus the original query.

The ideal presentation is a pipeline diagnosis:

```text
Document exists?                  YES
Satisfies explicit filters?       YES
Entered retrieval candidate set?  NO   ← first observed divergence
First-phase ranked?               —
Second-phase reranked?            —
Returned?                         NO
```

Not every stage can always be proven cheaply. The UI must distinguish:

- observed;
- inferred from authoritative engine evidence;
- unavailable.

If direct proof is unavailable, say so.

A forced/counterfactual diagnostic query may be used where supported, but it must
be clearly labelled as a rerun that differs from the original execution.

Never present counterfactual ranking values as proof that the document matched
the original query.

## 7.4 Too many / irrelevant results are matching

The guided flow should emphasize retrieval/query interpretation before ranking.

Useful evidence includes:

- effective query tree;
- tokenization/field scoping where available;
- filters;
- retrieval operator type;
- weakAnd/WAND limits;
- candidate counts;
- query transformations.

The likely debugging question is often “what did we retrieve?” rather than “why
did this score win?”

## 7.5 Vector/hybrid search looks wrong

For `nearestNeighbor` retrieval, expose:

- tensor field;
- query tensor name/type;
- distance metric where available;
- `approximate`;
- `totalTargetHits` / target-hit controls;
- `hnsw.exploreAdditionalHits`;
- distance/closeness features where configured;
- relevant filtering behavior;
- candidate counts.

Provide an explicit diagnostic action:

> **Compare with exact retrieval**

Run the equivalent query with `approximate:false` when the backend can do so
safely.

Report useful comparison metrics such as:

- overlap@k;
- recall@k when the exact run is a valid reference;
- hits entering/leaving top-k;
- rank movement;
- normal approximate latency;
- exact diagnostic latency.

The UI must warn that exact retrieval can be much more expensive.

## 7.6 The query is slow

The guided flow should progressively request:

1. ordinary query/search/fill timing;
2. trace timestamps;
3. matching profiling;
4. first-phase ranking profiling;
5. second-phase ranking profiling.

Vespa exposes targeted profiling using:

- `trace.profiling.matching.depth`;
- `trace.profiling.firstPhaseRanking.depth`;
- `trace.profiling.secondPhaseRanking.depth`;
- the aggregate `trace.profileDepth`.

The UI should render both tree and flat/top-self-time forms when available.

Example:

```text
Query execution                 31.4 ms

Container / query processing     1.8 ms
Matching                        12.9 ms
First-phase ranking              8.7 ms
Second-phase ranking             5.1 ms
Fill / summaries                 2.9 ms
```

Then drill down into an expensive phase:

```text
First-phase ranking              8.7 ms

bm25(title)                      3.8 ms
closeness(embedding)             2.9 ms
attribute(popularity)            0.1 ms
other expression work            1.9 ms
```

Do not compare diagnostic-profile latency directly with ordinary latency without
labelling the instrumentation overhead.

## 7.7 I want to understand the query

Show the query progressively:

- user-facing Search Lab state;
- canonical Search Kernel request;
- engine request/YQL where exposed;
- transformed query tree;
- content-node blueprint;
- search iterator tree.

The first view should explain structure visually.

The final view should expose raw evidence.

---

# 8. Analyze information architecture

The existing Analyze surface should become the home of W5 rather than adding a
separate top-level product.

Recommended structure:

```text
ANALYZE

Overview
Ranking
Matching
Performance
Vectors
Vespa internals
```

## 8.1 Overview

Start with the smallest useful summary:

```text
Query
  "wireless headphones"

Results
  10 returned

Ranking
  semantic-v2
  first phase → second phase
  no global phase

Retrieval
  hybrid lexical + nearest neighbor

Performance
  12.4 ms

Diagnostic hints
  Approximate vector retrieval targets 100 candidates.
```

Do not show a warning merely because an advanced engine feature is in use.
Diagnostic hints should identify facts worth investigating, not generate noisy
“best practice” advice.

## 8.2 Ranking

Contains:

- compact named-signal explanation;
- phase pipeline;
- per-hit evidence;
- hit comparison;
- raw feature inspector.

## 8.3 Matching

Contains:

- effective query;
- query tree;
- filters;
- candidate-generation information;
- blueprint;
- iterator tree at advanced depth.

## 8.4 Performance

Contains:

- normal timing;
- diagnostic timing;
- matching profiling;
- ranking profiling;
- fill/summary timing.

## 8.5 Vectors

Shown when vector retrieval/ranking is involved.

Contains:

- NN operator configuration;
- distance/closeness evidence;
- approximate/exact status;
- HNSW search controls;
- filtering interaction;
- exact-vs-approx comparison.

## 8.6 Vespa internals

Contains raw/near-raw engine evidence and explicit engine parameters.

This tab is intentionally allowed to be dense.

---

# 9. Backend contract

W5 must not make the frontend a parser/interpreter for Vespa semantics.

## 9.1 Keep ordinary search canonical

The normal Search Kernel search request/response remains the production semantic
contract.

Do not add unstable Vespa tracing payloads to every ordinary `SearchResponse`.

## 9.2 Add an explicit diagnostic operation

Introduce a dedicated diagnostic request path in Search Kernel/searchkerneld.

The exact schema/name is implementation work, but conceptually:

```text
QueryDiagnosticsRequest
  target / generation
  canonical query
  diagnostic intent
  requested evidence
  optional selected hit/document ids
  diagnostic limits
```

Possible typed diagnostic intents:

```text
EXPLAIN_HIT
COMPARE_HITS
MISSING_DOCUMENT
MATCHING
VECTOR_RETRIEVAL
PERFORMANCE
QUERY_TRANSFORMATION
DEEP_ENGINE
```

The backend maps these intents to engine-specific diagnostic controls.

Do not make the browser construct arbitrary Vespa query parameters as the
primary product API.

## 9.3 Diagnostic response

Conceptually:

```text
QueryDiagnostics
  request_identity
  target_identity
  normal_result_reference
  effective_query
  retrieval
  ranking
  timing
  hit_evidence[]
  warnings[]
  raw_engine_diagnostics?
```

The normalized response should include only concepts Search Kernel is willing to
support as a durable diagnostic contract.

## 9.4 Raw Vespa diagnostics are explicitly unstable

Vespa documents trace/explain/profiling output as subject to change.

Therefore:

- keep raw engine diagnostics in an engine-specific/opaque section;
- version or label their engine provenance;
- do not silently promote raw Vespa JSON field names into stable Search Kernel
  semantics;
- normalize only the subset we intentionally own;
- preserve raw payloads for expert inspection when practical.

## 9.5 Provenance is mandatory

Every piece of displayed evidence should have an identifiable provenance class:

- canonical Search Kernel request;
- normalized Search Kernel diagnostic;
- configured rank-profile feature;
- raw Vespa diagnostic;
- UI-derived comparison.

UI-derived comparison means arithmetic or presentation such as “largest
difference between A and B”; it must not be confused with engine-authored causal
explanation.

## 9.6 Incarnation safety

Diagnostics operate on the same generation-bound target semantics as ordinary
search.

A diagnostic rerun against a stale/recreated index must fail through the normal
canonical stale-incarnation behavior rather than silently running against the
new incarnation.

---

# 10. Friendly explanation rules

The approachable UI must follow strict evidence rules.

## 10.1 Allowed

The UI may:

- rename intentionally configured signals using known display metadata;
- group rank features by phase;
- compare numeric values;
- show rank movement;
- show whether a phase ran;
- summarize authoritative pipeline facts;
- calculate overlap@k from two returned result sets;
- identify the first stage for which authoritative evidence shows divergence.

## 10.2 Forbidden

The UI must not:

- infer the semantic meaning of arbitrary ranking-expression fragments;
- claim a feature “caused” a rank difference unless the configured expression
  establishes that relationship;
- claim a missing document “did not match” merely because it was not returned;
- treat approximate NN top-k as exhaustive matching;
- present a counterfactual diagnostic rerun as the original execution;
- reconstruct Search Kernel/Vespa query or ranking semantics in TypeScript;
- hide engine uncertainty behind generated natural-language confidence.

## 10.3 Optional future natural-language assistance

A future assistant could summarize diagnostic evidence in natural language, but
that is outside W5 closure.

If added later, it must cite/link every claim back to structured evidence and
must never replace the deterministic diagnostic view.

---

# 11. Rank-profile authoring implications

W3 owns rank-profile configuration; W5 consumes it.

W5 should encourage, but not require, profiles to expose meaningful named
functions for debugging.

A well-debuggable profile might define:

```text
function lexical_score() { ... }
function semantic_score() { ... }
function freshness_score() { ... }

match-features {
    lexical_score
    semantic_score
}

summary-features {
    freshness_score
}
```

Possible future W3 enhancement:

- optional human display labels/descriptions for selected explanation features.

Do not require Search Kernel's core semantic schema to adopt UI-only display
metadata unless the requirement proves broadly useful outside the web product.

---

# 12. Implementation slices

Implement W5 vertically.

## W5.0 — diagnostic transport and evidence model

Deliver:

- dedicated diagnostic operation;
- generation-bound target semantics;
- typed diagnostic intent;
- normalized-vs-raw evidence separation;
- real-engine fixtures/tests for trace, ranking features, and profiling.

No broad UI should be built before this seam is credible.

## W5.1 — Why this result?

Deliver:

- per-hit entry point;
- named match/summary feature display;
- phase-aware score display;
- compact explanation;
- raw evidence drill-down.

This is the first user-visible slice.

## W5.2 — compare hits

Deliver:

- two-hit selection;
- aligned named feature comparison;
- first/second/final score comparison;
- tensor-safe feature rendering;
- difference highlighting with no invented causality.

## W5.3 — guided query troubleshooting

Deliver intent-driven flows for:

- missing result;
- overmatching/irrelevant matching;
- query understanding.

Include query tree / blueprint evidence where available.

## W5.4 — query performance debugger

Deliver:

- timing hierarchy;
- trace timestamps;
- matching profiler;
- first-phase profiler;
- second-phase profiler;
- tree/flat representations.

## W5.5 — vector/hybrid diagnostics

Deliver:

- NN configuration inspection;
- distance/closeness evidence;
- approximate/exact comparison;
- overlap/recall analysis;
- filtering/candidate diagnostics.

## W5.6 — Vespa expert surface

Deliver:

- raw engine request/response;
- raw trace;
- blueprint;
- iterator tree;
- raw feature views;
- explicit low-level diagnostic controls where safe.

---

# 13. Verification

W5 requires real-engine evidence.

## 13.1 Unit/component tests

Cover:

- progressive disclosure;
- feature scalar/tensor rendering;
- no-explanation state;
- phase-presence/absence;
- hit comparison;
- provenance labels;
- diagnostic cost warnings.

## 13.2 Contract tests

Cover:

- diagnostic request/response canonical encoding;
- unknown/unsupported diagnostic intent;
- raw diagnostic payload isolation;
- stale-generation rejection;
- diagnostic error propagation.

## 13.3 Real-engine integration cases

At minimum:

1. lexical rank profile with named features;
2. first + second phase;
3. first + second + global phase if supported in the embedded path;
4. missing-result investigation;
5. approximate NN;
6. approximate vs exact NN comparison;
7. matching profiler;
8. first-phase profiler;
9. second-phase profiler;
10. restart/reopen followed by identical diagnostic query;
11. stale incarnation rejected after delete/recreate.

## 13.4 Browser E2E

A representative E2E should prove:

```text
open real index
  -> run query
  -> open Why this result?
  -> see authoritative named ranking evidence
  -> compare two hits
  -> run one guided diagnostic
  -> drill into Vespa internals
  -> reload/share state where appropriate
```

The browser test must use a real `searchkerneld` and real embedded engine path,
not fixture-only data.

---

# 14. Performance and safety constraints

- Normal search must not pay deep-diagnostic cost.
- Diagnostic response sizes must be bounded.
- Trace/profile depth must have safe server-side maxima.
- Large tensors require truncation/preview controls in the UI while preserving an
  explicit path to full data when feasible.
- Deep diagnostics must respect normal query timeout/cancellation semantics.
- Diagnostic reruns must never mutate index state.
- Raw engine diagnostics must be treated as potentially large and
  version-dependent.
- Sensitive query/document data must not be persisted merely to support the
  debugger unless a separate product decision explicitly introduces saved
  diagnostic sessions.

---

# 15. Explicit non-goals

W5 is not:

- a generic Vespa administration console;
- a distributed Vespa cluster debugger;
- a replacement for application-level observability;
- an offline relevance-evaluation suite;
- judgment-set management;
- NDCG/MRR experiment management;
- production A/B testing;
- a generic log viewer;
- distributed tracing across external services;
- an LLM that guesses why ranking happened;
- a frontend implementation of query/ranking semantics.

Those may become separate products or later milestones.

---

# 16. Milestone exit criteria

W5 is complete when all of the following are true:

- ordinary Search Lab remains uncluttered and does not enable expensive
  diagnostics by default;
- every result has a useful `Why this result?` path;
- approachable explanations use only authoritative exposed signals;
- users can compare ranking evidence between two hits;
- users can launch guided troubleshooting for missing results, overmatching,
  vector retrieval, slow queries, and query interpretation;
- retrieval and ranking are shown as separate concepts;
- first-phase, second-phase, global-phase, and fill behavior are represented
  accurately when present;
- vector diagnostics support approximate-vs-exact comparison where supported;
- query performance diagnostics can expose matching and ranking profiler data;
- diagnostic modes are opt-in and cost-tiered;
- the UI never fabricates explanation semantics for arbitrary rank expressions;
- every friendly abstraction has a direct evidence drill-down;
- expert users can access raw Vespa trace/plan/feature/profiling evidence;
- unstable Vespa diagnostic structures are not accidentally promoted into the
  stable Search Kernel search contract;
- diagnostic execution obeys canonical target/generation semantics;
- real-engine browser evidence covers the principal workflow.

---

# 17. Vespa capability references

The implementation should be grounded in the current Vespa behavior documented
by the upstream project, especially:

- Query API tracing and profiling:
  https://docs.vespa.ai/en/reference/api/query.html
- Phased ranking:
  https://docs.vespa.ai/en/ranking/phased-ranking.html
- Ranking basics / rank-profile functions:
  https://docs.vespa.ai/en/basics/ranking.html
- Rank feature reference:
  https://docs.vespa.ai/en/reference/ranking/rank-features.html
- Schema reference for match-features / summary-features:
  https://docs.vespa.ai/en/reference/schemas/schemas.html
- YQL nearest-neighbor annotations:
  https://docs.vespa.ai/en/reference/querying/yql.html
- Nearest-neighbor diagnostic/tuning guide:
  https://docs.vespa.ai/en/querying/nearest-neighbor-search-guide.html

When Vespa diagnostic output differs from this spec, prefer the actual supported
engine behavior and update this specification rather than adding frontend
heuristics.

---

# 18. Product principle to preserve

W5 should make Search Kernel feel simpler than Vespa without making it less
powerful.

The user journey should therefore be:

```text
search
  → explain
  → troubleshoot
  → inspect the search pipeline
  → inspect Vespa itself
```

not:

```text
learn Vespa
  → understand the debugger
  → maybe diagnose the search
```

That progressive path is the product.
