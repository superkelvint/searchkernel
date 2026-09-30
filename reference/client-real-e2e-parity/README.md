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

Integer-semantic fields (counts, totals, integer document fields, and related
values) require an integer JSON representation and exact equality. Booleans are
distinct. Other finite numeric fields use relative and absolute tolerance
`1e-6`; NaN and infinity never compare successfully.

Snapshots contain observable results from the shared showcase scenarios,
including search order and counts, document and tensor values, groups and
aggregations, ranking features, mutations, and structured errors. Each language
writes its snapshot only when `SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT` is set;
ordinary showcase runs remain self-contained. Run the complete gate with:

```bash
python3 tools/check-client-real-e2e-parity.py
```
