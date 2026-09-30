# First-party client real-engine parity

`corpus.json` is the shared schema, rank profiles, product documents, tensor
vectors, and coverage probe used by each full public-client E2E showcase. The
showcases keep their language-specific examples, but their primary index now
starts from this common corpus.

The corpus also defines language-neutral inputs for each shared typed-query
case. Rust, Python, TypeScript, PHP, and Ruby build those requests through their
public query facades and emit the consumed intent beside the ordered result
IDs. The checker rejects missing typed-query IDs or an emitted intent that
differs from the shared definition. The typed-query inventory includes
contains, phrase, range, boolean composition, IN, equality, fuzzy, WeakAnd,
WAND, weighted set, dot product, SameElement, geo location, and geo box.
`people` and `location`, plus the deterministic documents needed by SameElement
and geo queries, are part of the shared schema and corpus.

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

Numeric comparison preserves JSON integer and float kinds: integers require
exact equality, and an integer never compares equal to a float. Booleans remain
distinct from both. Finite floats use relative and absolute tolerance `1e-6`;
NaN and infinity never compare successfully. A small explicit set of result
paths whose domain type is floating (prices, scores, and floating aggregation
outputs) uses float semantics even when a client encoder writes an integral
float such as `3.0` as the JSON token `3`.

Snapshots contain observable results from the shared showcase scenarios,
including search order and counts, document and tensor values, groups and
aggregations, ranking features, mutations, and structured errors. Each language
writes its snapshot only when `SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT` is set;
ordinary showcase runs remain self-contained. Run the complete gate with:

```bash
python3 tools/check-client-real-e2e-parity.py
```
