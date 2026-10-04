# W5 — Search Understanding and Query Debugger Specification

**Status:** Proposed UX and product specification  
**Product:** Searchkernel Web  
**Milestone:** W5, immediately after W4 index operations and diagnostics  
**Primary UI reference:** Algolia-style progressive disclosure  
**Engine depth:** Vespa query, retrieval, ranking, and profiling diagnostics  
**Companion contract:** `W5 — Query Diagnostics Backend Contract.md`

---

# 1. Purpose

W5 turns Searchkernel Web from a search workbench into a search debugger without
turning ordinary search into a Vespa console.

The product must work for two very different users:

1. a developer who knows that a result looks wrong but does not know Vespa's
   execution model;
2. a search/Vespa engineer who needs the exact query tree, candidate-generation
   behavior, ranking features, execution plan, iterator tree, and profiler data.

The product principle is:

> **Start with the user's question. Reveal engine complexity only when it helps
> answer that question. Every friendly explanation must have a direct path to the
> underlying evidence.**

W5 is not a separate observability product. It is a progressive extension of
Search Lab.

---

# 2. User-facing mental model

The user-facing model has only three layers:

```text
Explain
   ↓
Investigate
   ↓
Inspect
```

Do not expose separate "diagnostic modes" or force users to understand cost
tiers. Cost is surfaced contextually on actions that require reruns.

## 2.1 Explain

Answer the immediate question with the smallest useful amount of evidence.

Typical entry points:

- **Why this result?**
- **Compare with result above**
- **Explain lens**

Explain stays in the result context.

## 2.2 Investigate

Guide the user through a problem:

- zero results;
- expected result missing;
- result ranked too high/low;
- too many/irrelevant results;
- vector/hybrid retrieval looks wrong;
- query is slow;
- query itself is confusing.

The flow is organized around findings, not engine components.

## 2.3 Inspect

Expose the search pipeline and raw evidence:

- query;
- retrieval;
- ranking;
- performance;
- vector internals;
- raw Vespa evidence.

Every technical term shown here must have a concise inline definition.

---

# 3. Search Lab interaction model

W5 should remain anchored in Search Lab.

Use a persistent split view:

```text
┌──────────────────────────────┬───────────────────────────────┐
│ Search + results             │ Evidence panel                │
│                              │                               │
│ #1 result                    │ Explain / Investigate /       │
│ #2 result                    │ Inspect content for the       │
│ #3 result                    │ current selection             │
│ #4 result                    │                               │
│ ...                          │                               │
└──────────────────────────────┴───────────────────────────────┘
```

Selecting a hit changes the evidence panel without leaving the result list.

Pinning a second hit starts a comparison.

Provide a direct shortcut:

> **Compare with result above**

because "why is #4 below #3?" is a natural ranking-debugging question.

The existing Analyze route may remain as an expanded/full-page view of the same
evidence model, but it must not be required for ordinary investigation.

---

# 4. Explain layer

## 4.1 Why this result? — baseline state

The most common case must work even when the rank profile exposes no named
explanation functions.

Baseline panel:

```text
WHY THIS RESULT?

Result #4
Ranking profile: product-search

What we know
────────────────────────────────

Returned position                 #4
Final relevance                   0.782

Ranking path
Retrieval                         completed
Initial ranking                   rank #11
Reranking                         selected
Final ordering                    #4

Explanation signals
────────────────────────────────

This profile does not expose named ranking signals,
so Search Kernel cannot safely say which parts of the
ranking expression mattered.

[Make this profile easier to explain]

Technical evidence
[Show ranking phases]
[Show available rank features]
```

This is not an error state. It is a useful explanation with an explicit limit.

## 4.2 Make this profile easier to explain

The no-signals state must offer a concrete path forward.

Example:

```text
Make this profile easier to explain

Expose meaningful intermediate functions from your
rank profile as match-features or summary-features.

function lexical_score() {
    expression: bm25(title) + bm25(body)
}

function semantic_score() {
    expression: closeness(field, embedding)
}

match-features {
    lexical_score
    semantic_score
}

[Copy example] [Open rank profile]
```

W3 owns editing the rank profile. W5 should deep-link into the relevant W3
surface when editing is available.

## 4.3 Why this result? — rich-signal state

When a profile exposes named signals, show values with context, not adjectives.

Do **not** say:

- "Strong text relevance";
- "Popularity helped";
- "Weak semantic score";

unless there is an explicit, defined baseline that makes the label true.

Prefer:

```text
WHY THIS RESULT?

Result #4 of 10
Ranking profile: semantic-v2

Ranking signals
────────────────────────────────

Text relevance        3.91
2nd highest among these 10 returned results

#1  4.43  ━━━━━━━━━━━━━━━━━━━●
You 3.91  ━━━━━━━━━━━━━━━━●━━━
#10 0.62  ━━●━━━━━━━━━━━━━━━━━

Semantic similarity   0.84
5th highest among these 10 returned results

Freshness             0.54
8th highest among these 10 returned results

[Show ranking path] [Inspect evidence]
```

Any percentile/rank label must state its comparison set. Do not imply corpus-wide
or matched-set percentiles when only returned hits were observed.

## 4.4 Explain lens

Add an optional **Explain lens** for scanning the result set.

When enabled, each hit may show a compact subset of intentionally named signals:

```text
#1 Sony ...
   lexical  █████████
   semantic ███████

#2 Bose ...
   lexical  ██████
   semantic ████████
```

The lens is for pattern recognition, not for proving causal importance.

Users can configure which named signals are visible if the profile exposes many.

Normal search remains uncluttered when the lens is off.

## 4.5 Matched-term evidence

Where the engine/backend exposes authoritative match information, show intuitive
lexical evidence such as:

```text
Matched terms

"wireless"   title
"headphones" title, description
```

Highlight matched terms in snippets when that evidence is available.

Do not infer field-level matching from string coincidence in the frontend.

---

# 5. Compare results

## 5.1 Interaction

A user can pin two hits or choose **Compare with result above**.

Keep both hits visible in the result list while the evidence panel shows the
comparison.

## 5.2 Default comparison

Do not highlight the largest raw numeric difference across heterogeneous feature
scales.

Instead show:

- which hit each signal favours by its position within the current returned set;
- each hit's result-set rank for that signal;
- raw values as secondary evidence.

Example:

```text
WHY IS A ABOVE B?

                         Result A       Result B

Lexical score            2nd / 10       7th / 10
                         favors A

Semantic score           6th / 10       1st / 10
                                         favors B

Popularity               4th / 10       5th / 10
                         similar

Initial rank             #3             #17
Reranked                 yes            yes
Final position           #3             #8

[Show raw values]
[Inspect ranking expression]
```

Do not use color alone to communicate favour/direction.

## 5.3 Causality rule

The comparison may say:

- "A has a higher lexical signal";
- "B has a higher semantic signal";
- "A was ranked #3 after first phase";

but must not say:

- "A won because lexical mattered more";

unless the configured ranking expression and available evidence establish that
relationship.

---

# 6. Investigate layer

The main entry point is:

> **What's wrong with this search?**

Initial choices:

- I got zero results.
- A result I expected is missing.
- A result is ranked too high or too low.
- Too many or irrelevant results are matching.
- Vector/hybrid search looks wrong.
- The query is slow.
- I want to understand the query.

The UI should land on a **finding** and make the engine tabs/evidence secondary.

---

# 7. Zero-results flow

Zero results is a first-class W5 workflow.

## 7.1 Entry state

When a query returns zero hits, Search Lab should offer:

> **Find out why**

Do not require the user to open Analyze manually.

## 7.2 Controlled elimination experiments

The backend may generate safe diagnostic variants that remove one constraint at
a time.

Example:

```text
NO RESULTS

Let's find where the result set became empty.

Original query
  "wireless headphones"
  region = "EU"
  in_stock = true
                                  0 results

Diagnostic experiments
────────────────────────────────

Remove in_stock                  42 results
Remove region                   117 results
Remove "wireless"               18 results
```

A finding may then say:

> Removing `in_stock = true` is sufficient to produce results.

This is evidence from a rerun, not a claim about logical causality beyond that
experiment.

The backend, not the browser, must construct valid query variants because
removing clauses can change semantics for non-monotonic/top-k operators.

## 7.3 Next actions

Allow the user to:

- inspect the exact variant;
- apply the variant into the experiment lane;
- inspect the query tree;
- inspect filters;
- return to the original query.

---

# 8. Missing-result flow

## 8.1 Selecting the expected document

Do not require a document ID.

Allow:

- document ID;
- browse/search by title or configured fields;
- paste a known URL or unique field value when the index supports it.

The selected document is clearly identified before diagnostics run.

## 8.2 Finding-oriented output

Prefer:

```text
EXPECTED RESULT: "Bose QuietComfort Ultra"

Finding
────────────────────────────────
The document is excluded when the region = "EU"
filter is applied.

Evidence
────────────────────────────────
Document exists                         yes
Query without region filter returns it  yes
Query with region = "EU returns it      no

[Inspect filter]
[Open experiment]
```

When the backend can prove deeper pipeline state, show it:

```text
Document exists?                  YES
Satisfies explicit filters?       YES
Entered retrieval candidate set?  NO   ← first observed divergence
Initial ranking?                  —
Reranking?                        —
Returned?                         NO
```

Each row must distinguish:

- observed;
- calculated from observed values;
- unavailable.

Avoid a separate, noisy badge vocabulary for every provenance detail.

## 8.3 Counterfactual ranking

A forced diagnostic may evaluate ranking signals for a document that did not
enter the original candidate set.

It must be visually labelled:

> **Counterfactual diagnostic rerun — not the original execution**

Never use counterfactual feature values as proof that the document originally
matched.

---

# 9. Ranking-problem flow

When the user chooses "A result is ranked too high or too low":

1. select the result;
2. optionally select the result it should be near/above/below;
3. show the ranking path;
4. show named-signal positions across the current result set;
5. offer deeper feature/phase evidence;
6. offer an experiment.

Do not begin with a full rank-feature table.

---

# 10. Too-many/irrelevant-results flow

Start with query interpretation and retrieval rather than ranking.

Useful findings include:

- a filter was not applied as expected;
- a term is matching a broader field set than expected;
- the effective query is OR-like where the user expected AND-like behavior;
- a top-k retrieval operator is producing a broad candidate set;
- query transformation changed the apparent structure.

A friendly query rendering is desirable, for example:

```text
Matches ANY of:
  title contains "wireless"
  body contains "wireless"

AND

category is "headphones"
```

This rendering must be generated from backend-owned query semantics. The
frontend must not independently interpret Vespa query syntax.

---

# 11. Vector/hybrid retrieval flow

For nearest-neighbor/hybrid queries, start with friendly concepts:

```text
Vector retrieval

Approximate search        on
Candidate target          100
Returned                  10
Distance metric           angular

[Compare with exact retrieval]
```

Technical details can drill into:

- tensor field;
- query tensor;
- `approximate`;
- `targetHits`;
- `hnsw.exploreAdditionalHits`;
- distance/closeness;
- filtering interaction;
- candidate counts.

## 11.1 Exact comparison

When safe, support:

> **Compare with exact retrieval**

Show:

- overlap@k;
- hits entering/leaving top-k;
- rank movement;
- approximate latency;
- exact diagnostic latency.

Only call the metric "recall@k" when the exact run is a valid reference for that
claim.

Warn clearly that exact search may be much more expensive.

---

# 12. Slow-query flow

Begin with the least invasive evidence:

```text
Query time

Query processing      1.8 ms
Matching             12.9 ms
Initial ranking       8.7 ms
Reranking             5.1 ms
Fill / summaries      2.9 ms
```

Then offer targeted reruns:

> **Profile matching — reruns query with tracing**

> **Profile ranking — reruns query with profiling; slower**

The user should never have to select `trace.profileDepth` just to answer "why
is this slow?"

Inspect can expose the raw Vespa controls and profiler trees.

Diagnostic timing must be labelled separately from normal-run timing.

---

# 13. Experiment lane

Diagnosis should lead naturally to a safe "what if?" loop.

W5 includes a lightweight experiment lane, not a full relevance-evaluation
platform.

Supported experiments may include:

- change rank profile;
- add/remove a filter;
- change supported query input;
- adjust vector target-hit controls;
- switch approximate vs exact where valid.

Example:

```text
ORIGINAL                    EXPERIMENT

semantic-v2                 semantic-v3
targetHits: 100             targetHits: 300

────────────────────────────────────────

Result overlap               7 / 10

Sony XM6                     #1 → #3
Bose QC                      #4 → #2
AirPods Max                  new → #5

Latency                      8.2 ms → 12.7 ms
```

Every experimental result must be clearly labelled:

> **Experimental rerun**

The user can return to the original run at any time.

W5 does not include judgment sets, NDCG/MRR, statistical evaluation, or
production A/B testing.

---

# 14. Inspect layer

Inspect exposes the actual search pipeline:

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

Recommended sections:

- Query
- Retrieval
- Ranking
- Performance
- Vectors
- Vespa internals

These are evidence sections, not the primary way a user chooses a debugging task.

## 14.1 Inline definitions

Every engine-specific term must have a short definition available in place.

Examples:

- **weakAnd** — retrieves a bounded set of likely best lexical matches instead of
  exhaustively scoring every logical match.
- **target hits** — the requested candidate count for a top-k retrieval operator.
- **second phase** — reranking applied only to a configured subset of top initial
  results.

Definitions should be concise and should not hide the raw term from expert users.

## 14.2 Query tree readability

Provide two representations where possible:

1. backend-generated friendly rendering;
2. technical/raw tree.

Example:

```text
Friendly
Matches title or body for "wireless headphones"
AND category = "audio"

Technical
AND
├── weakAnd(...)
└── category:"audio"
```

## 14.3 Vespa internals

Expert view should expose, when supported:

- raw Vespa request;
- raw Vespa response;
- trace;
- `trace.level`;
- `trace.explainLevel`;
- query blueprint;
- iterator tree;
- raw match features;
- raw summary features;
- `ranking.listFeatures`;
- matching profiler;
- first-phase profiler;
- second-phase profiler.

Raw Vespa output remains engine-specific and is not itself a stable Search Kernel
semantic contract.

---

# 15. Findings and evidence

W5 separates **findings** from **evidence**.

Example:

```text
Finding

The expected document disappears when
region = "EU" is applied.

Evidence

Without region filter      returned
With region filter         not returned

[Inspect exact requests]
```

A finding is a deterministic presentation of evidence, not a free-form inference
engine.

## 15.1 Evidence labels

Use one simple provenance vocabulary:

- **Profile** — intentionally exposed by the rank profile/schema.
- **Engine** — reported by Search Kernel/Vespa execution.
- **Calculated** — deterministic arithmetic/comparison performed by the UI or
  normalized backend.

Observed/unavailable state should be represented directly in the data row rather
than layered into a second badge taxonomy.

Keep provenance visually quiet in Explain/Investigate and explicit in Inspect.

---

# 16. Rerun consistency

Diagnostic and experimental reruns may not reproduce the original result set
because of:

- index mutations;
- index generation changes;
- ANN behavior;
- timing-sensitive state;
- different diagnostic instrumentation.

The UI must detect and show divergence.

Example:

> **This diagnostic rerun differs from the original search: 2 of the top 10
> results changed. Evidence below is from the diagnostic rerun.**

Never silently attach rerun evidence to the original result list.

Generation-bound target semantics remain mandatory.

---

# 17. Loading, empty, error, and unsupported states

These are part of W5 closure.

## 17.1 Loading

For diagnostic reruns:

- keep the original results visible;
- show which evidence is being collected;
- allow cancellation where backend semantics support it;
- do not blank the evidence panel.

Example:

```text
Collecting ranking evidence…
Original search results remain visible.
```

## 17.2 No named explanation features

Use the useful baseline state from Section 4.1, not an empty/error panel.

## 17.3 Unsupported capability

Example:

> This Vespa/Search Kernel version does not expose iterator-plan diagnostics for
> this query.

Offer whatever lower-depth evidence remains available.

## 17.4 Timeout

Example:

> Ranking profiling exceeded the diagnostic timeout. The original search was not
> affected.

Offer a narrower rerun or lower profiling depth if supported.

## 17.5 Stale incarnation

Use canonical Search Kernel stale-generation behavior and tell the user the index
was recreated/reopened under a new generation.

Do not silently rerun against the new incarnation.

## 17.6 Partial evidence

A diagnostic may succeed partially.

Show available sections and mark only the unavailable evidence, rather than
failing the whole investigation.

---

# 18. Accessibility

W5 must not encode meaning only in diagrams, position, or color.

Requirements:

- all pipeline stages are represented as navigable text/structure;
- tree views support keyboard expansion/collapse;
- screen readers receive stage names, status, counts, and relationships;
- compare views include textual "favours A / favours B / similar" labels;
- Explain lens bars include numeric/text equivalents;
- focus remains stable when result selection changes;
- loading updates use appropriate live-region behavior without excessive chatter.

Accessibility applies to technical Inspect views as well as friendly layers.

---

# 19. Sharing and diagnostic bundles

W5 needs explicit sharing semantics.

## 19.1 Shareable URL

A shareable Search Lab/diagnostic URL may contain only non-sensitive,
user-approved query state that is already part of the normal URL-backed Search
Lab model.

Do not put into URLs by default:

- document bodies;
- tensors;
- raw diagnostic payloads;
- credentials;
- arbitrary query feature values that may be sensitive.

## 19.2 Diagnostic bundle

Provide an explicit export/copy action for collaboration/support.

A diagnostic bundle may include:

- Search Kernel version;
- Vespa/native version;
- target/index identity excluding secrets;
- query/ranking configuration;
- canonical request;
- normalized diagnostic evidence;
- optional raw Vespa evidence;
- rerun/divergence metadata.

The user chooses whether raw document fields/query feature values are included.

## 19.3 Copy as curl

Where the public diagnostic API supports it, provide **Copy as curl** using the
Search Kernel/searchkerneld API, not a private browser-only endpoint.

Secrets/auth headers must never be embedded automatically.

---

# 20. Diagnostic hints

Hints must be based on observed anomalies or comparisons, not merely on the
presence of an advanced feature.

Good:

> Only 37 candidates remained after filtering from a target of 100.

Good:

> Exact retrieval found 4 top-10 results that approximate retrieval missed.

Bad:

> Approximate vector retrieval is enabled.

Bad:

> This profile uses second-phase ranking.

A configuration fact is evidence, not automatically a warning.

---

# 21. Implementation order

W5 should be implemented vertically around user value, and UX prototyping should
not wait for the complete diagnostic transport.

Captured real Vespa traces/fixtures may be used for early UI/task testing.

## W5.0 — UX prototypes and evidence fixtures

Prototype with captured real-engine evidence:

- no-signals Why this result?;
- rich-signals Why this result?;
- zero-results flow;
- missing-result flow;
- persistent split view.

Run task-based usability tests before freezing the full contract.

## W5.1 — diagnostic transport and evidence model

Implement the companion backend contract:

- generation-bound diagnostic request;
- findings/evidence model;
- normalized-vs-raw engine evidence;
- bounded diagnostic controls.

## W5.2 — Why this result?

Ship:

- persistent evidence panel;
- useful no-signals state;
- named-signal context across returned hits;
- explain lens;
- matched-term evidence where available;
- "Make this profile easier to explain."

## W5.3 — zero-results and missing-result investigation

Ship these before broad hit-comparison work because they provide value even when
rank profiles expose few named features.

## W5.4 — experiment lane

Ship controlled query/ranking/vector reruns with explicit original-vs-experiment
comparison and divergence labelling.

## W5.5 — hit comparison and ranking investigation

Ship:

- pin/compare;
- compare-with-result-above;
- signal position/direction;
- phase evidence;
- raw values.

## W5.6 — performance and vector diagnostics

Ship:

- timing drill-down;
- profiling reruns;
- ANN/exact comparison;
- retrieval diagnostics.

## W5.7 — Inspect / Vespa expert surface

Ship:

- query tree;
- blueprint;
- iterator tree;
- raw feature views;
- profiler trees;
- raw Vespa request/response;
- low-level controls where safe.

---

# 22. Usability acceptance

W5 exit criteria include task success, not only correctness constraints.

Test with developers who understand search at a normal application-engineering
level but are not Vespa specialists.

Representative tasks:

1. Explain what is known about why a returned hit is #4 when the rank profile has
   no named explanation functions.
2. Identify which query constraint is sufficient to turn a zero-result query into
   a non-zero-result query.
3. Determine the first observed stage at which an expected document disappears.
4. Compare two adjacent hits and identify which named signals favour each hit
   without interpreting raw feature scale as importance.
5. Determine whether approximate NN retrieval is changing the top-10 result set.
6. Identify the dominant phase of a slow query and reach the underlying profiler
   evidence.

Success criteria:

- users can begin each task from Search Lab without knowing a Vespa trace
  parameter;
- the correct finding is reached without assistance;
- users can distinguish original execution from diagnostic/experimental reruns;
- users can reach the raw evidence when asked;
- no test requires learning the five-stage Vespa ranking vocabulary before the
  first useful answer appears.

Track completion time during testing, but do not encode an arbitrary fixed minute
threshold until baseline usability data exists.

---

# 23. Engineering correctness constraints

The UX above relies on the companion backend contract.

Non-negotiable rules:

- frontend does not reconstruct Search Kernel/Vespa query semantics;
- friendly query rendering comes from backend-owned semantics;
- ordinary search does not pay deep-diagnostic cost;
- diagnostic queries are opt-in reruns;
- arbitrary rank expressions are not given invented semantic explanations;
- counterfactual reruns are always labelled;
- approximate top-k is not presented as exhaustive matching;
- raw Vespa diagnostics remain engine-specific/unstable;
- target/generation semantics apply to every diagnostic rerun;
- large tensors and diagnostic payloads are bounded;
- diagnostic reruns never mutate index state.

See `W5 — Query Diagnostics Backend Contract.md` for the transport and evidence
model.

---

# 24. Explicit non-goals

W5 is not:

- a generic Vespa administration console;
- a distributed Vespa cluster debugger;
- a generic log viewer;
- a replacement for application observability;
- an offline relevance-evaluation suite;
- judgment-set management;
- NDCG/MRR experiment management;
- production A/B testing;
- an LLM that guesses why ranking happened;
- a frontend implementation of query/ranking semantics.

The experiment lane is intentionally lightweight and query-local.

---

# 25. Milestone exit criteria

W5 is complete when:

- Search Lab remains simple with diagnostics closed;
- Explain → Investigate → Inspect is the visible mental model;
- evidence appears in a persistent result-adjacent panel;
- Why this result? is useful even with no named rank-profile signals;
- the UI teaches users how to make a rank profile more explainable;
- named signal values always have an explicit comparison context;
- Explain lens supports scanning multiple hits;
- adjacent-hit comparison is one action away;
- zero results has a guided controlled-elimination flow;
- expected documents can be selected without requiring document IDs;
- missing-result diagnosis reports findings backed by explicit evidence;
- query experiments are clearly separated from original execution;
- diagnostic rerun divergence is visible;
- vector workflows can compare approximate and exact retrieval where supported;
- slow-query workflows progressively request profiler evidence;
- every friendly abstraction drills into authoritative evidence;
- technical terms have inline definitions;
- loading, timeout, unsupported, partial-evidence, and stale-generation states are
  designed and tested;
- tree/pipeline/compare UIs meet keyboard and screen-reader requirements;
- users can export a diagnostic bundle and copy a reproducible public-API request
  without leaking credentials;
- task-based testing with non-Vespa-specialist developers demonstrates that the
  primary investigations can be completed without assistance;
- expert users can still reach raw Vespa trace/plan/feature/profiling evidence.

---

# 26. Product principle to preserve

The journey is:

```text
search
  → explain what we know
  → investigate the user's problem
  → experiment safely
  → inspect the search pipeline
  → inspect Vespa itself
```

not:

```text
learn Vespa
  → understand the debugger
  → maybe find the problem
```

That progressive path is the product.
