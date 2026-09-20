# Phase 9 additive native ABI contract

Phase 9 keeps `SK_NATIVE_ABI_VERSION` at 12. Every existing public symbol and
signature remains unchanged; this phase adds only opaque, domain-specific
handles and typed operations.

The frozen additions are:

- nearest-neighbor options for distance threshold and explore-additional-hits;
- grouping max-groups, precision, ordering, regex filtering, sum/min/max,
  standard deviation, xor, quantiles, and summary-hit aggregation;
- arithmetic document updates with add, subtract, multiply, and divide;
- a copied raw Vespa sort specification for relevance, rank-feature, and
  missing-placement lowering.

The existing `in` and legacy-feature predicate query paths are covered by
real-engine tests and remain unchanged. Same-element execution is covered for
`array<string>` and object arrays represented by the portable schema model,
including relative member fields and `elementFilter`. Geo location and
bounding-box execution are covered with real position documents, including an
inclusive boundary case. Predicate documents accept ordinary features and the
canonical Vespa Slime expression representation, including range nodes and
AND/OR/NOT composition; predicate schema configuration exposes arity and the
supported range-index bounds.

Raw predicate JSON is accepted only as the documented Vespa predicate Slime
representation and is annotated natively before indexing. Prefix, substring,
and suffix query operators are not part of the portable query IR. Collection
forms outside the supported arrays, sparse or mixed tensors, unsupported
field-path forms, and unrepresented update forms remain deferred.

The Phase 9 script is cumulative: it first runs the earlier milestone
verifiers, then runs five selected real-engine `typed_query_*` adapter slices
and the specialized native-query checks listed in the script. The independent
adapter test filter contains thirteen typed-query tests; that count is not a
claim that the cumulative Phase 9 verifier is green when an earlier milestone
gate fails.

The native boundary uses opaque handles and primitive arguments only. Text is
copied by the adapter, options and filters have exactly-once destruction, and
all operational failures are returned through the existing error buffer.
