# First-party client real-engine parity

`corpus.json` is the shared schema, rank profiles, product documents, tensor
vectors, and coverage probe used by each full public-client E2E showcase. The
showcases keep their language-specific examples, but their primary index now
starts from this common corpus.

`parity_scenarios` names the observations that every showcase writes for
`tools/check-client-real-e2e-parity.py`. The checker launches the Rust, Python,
TypeScript, PHP, and Ruby real-engine E2Es, then compares each observation's
structure and values. Integer values, booleans, strings, and ordered result IDs
must match exactly. Floating-point values may differ by a relative or absolute
tolerance of `1e-6`.

The snapshots include observable search order and counts, facets, nearest
neighbor IDs and scores, two-phase reranking, portable error details, a
patch/delete cycle, and a persisted document read after reopen. Each language writes its snapshot only when
`SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT` is set; ordinary showcase runs remain
self-contained. Run the complete gate with:

```bash
python3 tools/check-client-real-e2e-parity.py
```
