# First-party client real-engine parity

`corpus.json` is the shared schema, rank profiles, product documents, tensor
vectors, and coverage probe used by each full public-client E2E showcase. The
showcases keep their language-specific examples, but their primary index now
starts from this common corpus.

`scenarios` is the single inventory for the five public-client showcases and
`tools/check-client-real-e2e-parity.py`. The `non_comparable` entries explain
the scenarios that Rust's public facade cannot currently express. Each
showcase writes a normalized observation for every other scenario, and the
checker fails if any client is missing one. It compares each observation's
structure and values.
Integer values, booleans, strings, and ordered result IDs must match exactly.
Floating-point values may differ by a relative or absolute tolerance of
`1e-6`.

Snapshots contain observable results from the shared showcase scenarios,
including search order and counts, document and tensor values, groups and
aggregations, ranking features, mutations, and structured errors. Each language
writes its snapshot only when `SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT` is set;
ordinary showcase runs remain self-contained. Run the complete gate with:

```bash
python3 tools/check-client-real-e2e-parity.py
```
