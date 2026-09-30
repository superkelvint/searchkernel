# First-party client real-engine parity

`corpus.json` is the shared schema, rank profiles, product documents, tensor
vectors, and coverage probe used by each full public-client E2E showcase. The
showcases keep their language-specific examples, but their primary index now
starts from this common corpus.

`scenarios` is the required inventory for all five public-client showcases and
`tools/check-client-real-e2e-parity.py`. Every client must emit an observation
for every scenario. A language limitation is recorded under
`client_scenario_contract` for that client, scenario, and specific case, and
every client must have an explicit classification for each case in that
scenario. Runtime-capable clients must record the rejected input; a
`not_expressible` client must give a concrete reason and omit that impossible
runtime observation. Each snapshot repeats its exact checked classification
alongside its observations. The checker validates runtime results, compares
the remaining shared observations, and fails on absent clients, absent
scenarios, uncontracted skips, or differing comparable observations.

There is no global exclusion list. The only currently declared capability
cases are the Rust negative-limit and negative-offset construction cases in
`builder_misuse`; Rust accepts `u32` for both values, so negative values cannot
be represented through its public builder. The checker pins this per-client
matrix and rejects other or changed classifications. The other four clients
must execute the negative inputs and observe runtime rejection.

Integers compare exactly and never enter float tolerance. Booleans are distinct
from integers. A strict path schema marks the corpus's float64 fields, floating
aggregations, scores, and ranking features; on those paths the checker accepts
integral JSON numbers as floating values because JavaScript JSON serialization
does not preserve a `3.0` spelling. Two finite float-semantic values use
relative and absolute tolerance `1e-6`. On every other path, JSON integers and
floating numbers are distinct. NaN and infinity never compare successfully.

Snapshots contain observable results from the shared showcase scenarios,
including search order and counts, document and tensor values, groups and
aggregations, ranking features, mutations, and structured errors. Each language
writes its snapshot only when `SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT` is set;
ordinary showcase runs remain self-contained. The PHP E2E builds its common
`IndexDefinition` from this corpus and adds only the `people` and `location`
fields used by its existing typed-query showcase. A PHP regression test checks
that every shared field type and capability, tensor type, rank profile,
query-input type, match feature, phase expression, and rerank count is consumed.
Run the complete gate with:

```bash
python3 tools/check-client-real-e2e-parity.py
```
