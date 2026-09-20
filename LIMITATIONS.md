# Vespa compatibility limitations

Search Kernel is a thin wrapper around the pinned Vespa engine, but its public
contract is intentionally smaller than Vespa's complete query, schema, and
document model. This file records places where the wrapper is not a transparent
Vespa implementation. Unsupported behavior must remain an explicit error; it
must not be silently approximated.

## Query semantics

- `text(...)` is implemented as plain analyzed text with Vespa's default
  `WeakAnd` semantics. It is not a phrase. Use `phrase(...)` when phrase
  semantics are required.
- `in(...)` uses Vespa's native `InTerm` node for string and integer attribute
  values. It is not lowered to an OR of equality terms.
- `userInput(...)` and `userQuery(...)` implement the pinned adapter's reduced
  grammar: non-empty terms, quoted phrases without escapes, optional simple
  `field:term`/`field:"phrase"` selectors, leading `-` negation,
  parentheses, explicit `AND`/`OR`, and implicit adjacent-term `WeakAnd`.
  `text(...)` is the explicit native `WeakAnd` form. Vespa's complete
  `userInput()`/`userQuery()` grammar is not reproduced. Escaping, weights and
  boosts, range syntax, prefix/suffix/wildcard/fuzzy syntax, parser modes such
  as `{grammar:...}`, and unsupported function/composite forms are rejected
  rather than reinterpreted as terms. The rejection cases are executable
  adapter tests.
- `NonEmpty(...)` validates the lowered child and rejects statically empty
  boolean operators, then lowers to that child. It does not reproduce all of
  Vespa's parser-generated null/empty markers.
- `alternatives(...)` currently accepts term children only. More general
  alternatives are rejected.
- Labels are allocated by the adapter and exported as generated
  `vespa.label.<label>.id` rank properties. Callers must not provide those
  internal properties.
- WAND and nearest-neighbor execution exposes the currently supported Vespa
  controls through typed annotations. Unknown controls are rejected rather than
  dropped.

## Schema and document model

- The native document ABI supports scalar values, dense tensors,
  `array<string>`, supported `array<struct>` values, position values, and
  predicate values. Array-of-struct member queries are exposed as relative
  member fields under the parent element and are not a general nested
  struct/map model. SameElement supports those represented arrays and native
  `elementFilter`; unsupported member types and arbitrary dotted paths are
  rejected.
- Position fields publish the Vespa-compatible derived zcurve attribute inside
  the native adapter, and geo location/bounding-box queries execute in Vespa.
  Bounding-box edges are inclusive in the verified contract. Search Kernel
  does not implement geo matching in Rust.
- Predicate documents support ordinary feature values and the canonical Vespa
  predicate Slime JSON expression form, including numeric range constraints and
  AND/OR/NOT composition. Predicate fields require a positive `arity` and the
  adapter emits the supported Vespa range-index bounds. The wrapper does not
  implement a separate predicate evaluator or the full Vespa predicate parser;
  unsupported expression shapes are rejected, and native range annotations are
  generated only for the supported signed-integer range representation.
- The nearest-neighbor path is proven for dense tensors and the default
  execution slice. Its supported target-hit, approximate, distance-threshold,
  and exploration annotations lower to native Vespa controls, but the full
  Vespa HNSW tuning surface is not exposed.
- Sparse and mixed tensors remain unsupported in the current document and
  query-input paths.

## Result and ranking model

- Match features cross the Vespa boundary as `double` or borrowed opaque data;
  tensor decoding is a higher-layer capability and is not implied by the
  Vespa feature container.
- Grouping syntax/source forms: structured grouping is accepted only through
  the current portable IR. `RawGroupingSource` remains unsupported because the
  repository has no parser that can safely lower raw Vespa grouping text.
  Unsupported raw source is rejected with a structured error; it is not
  interpreted as a second grouping path. Status: intentional limitation.
- Grouping operations and topology: structured `all`, `each`, root `where(true)` /
  `where($query)`, max/unlimited max, precision, order, aliases, labels,
  regex/boolean/range filters, keep, nested groups, and the implemented linear
  branch are lowered through the native grouping plan. Vespa's parallel
  grouping branches still cannot be represented by one `GroupingPlan`; a
  request requiring multiple independent branches is rejected instead of being
  merged in Rust. User-visible impact: some valid branching programs do not
  execute. Reason/status: native ABI topology is intentionally linear; additive
  multi-plan result composition remains open.
- Grouping expressions and functions: finite scalar literals, attributes,
  aggregation references, unary negation, arithmetic, and the native `cat`,
  `reverse`, `normalize`, `strlen`, and `xor` functions are typed native
  expressions. `Member`, `MapLookup`, `Bucket`, and the remaining validated
  Vespa function names still require dedicated native constructors. User-visible
  impact: those expressions fail with a structured unsupported error. Reason/status:
  the pinned Vespa expression classes have different typed constructors; no
  expression is evaluated in Rust or Python.
- Grouping aggregations and output identity: count, sum, average, min, max,
  xor, standard deviation, quantiles, summary, root aggregations, aliases,
  references, labels, and aggregation-expression operands use typed native
  handles. Native Vespa still determines the values and ordering. User-visible
  impact: unsupported expression forms are rejected before execution. Status:
  implemented for the documented native expression subset.
- Grouped summaries: `summary()` is represented in the native result path, but
  grouped-hit/docsum ownership and external document-id fidelity are not yet
  proven for every summary class. Users may receive incomplete grouped-hit
  results or an explicit native error. Status: known implementation gap under
  real-Vespa verification.
- Grouping continuations: continuation tokens are not currently represented
  end-to-end in the linear grouping plan/native ABI or portable result. User-
  visible impact: requests requiring continuation round-trips are rejected or
  return no continuation. Reason/status: the pinned embedded `Grouping` API
  exposes level processing but no public continuation setter/accessor for this
  ABI seam; requires a typed native continuation addition.
- Group ordering, max, and precision: the native plan exposes the currently
  wired ordering, maximum-group, precision, and unlimited-max controls, but
  unsupported expression forms and branching topology can still prevent a
  valid V2 request from lowering. Status: partial implementation.
- Result tree surface: recursive groups, keys, aggregation values, and the
  canonical root are exposed for the implemented plan shape. Vespa's complete
  result-tree and grouping-output surface is not exposed. Status: partial
  wrapper contract.
- Timeout and ANN-timeout degradation bits are mapped when Vespa supplies them,
  but deterministic acceptance relies on real match-phase degradation rather
  than wall-clock timing tests.
- The SearchReply issue vector is preserved and exposed through bounded issue
  accessors. The frozen verifier does not fabricate a Vespa issue; no pinned
  deterministic query currently produces one reliably enough for acceptance.

## Versioning

Behavior is pinned to the Vespa revision recorded in
`reference/vespa/PINNED_VESPA.json`. Changes in upstream Vespa behavior require
new source inspection and a verifier update; they are not inferred from API
names alone.

## YQL and query controls

- YQL grammar: Search Kernel uses the existing low-level parser, not Vespa's
  complete query-language parser. The implemented subset covers the current
  `select ... where` source form, boolean predicates, `contains`, `phrase`,
  equality/range forms, selected annotations, and the typed `ORDER BY` path.
  Other Vespa YQL grammar, query operators, and annotations are rejected.
  User-visible impact: valid full-Vespa YQL can fail with a structured parse
  error. Status: intentional embedded-engine subset.
- YQL annotations and properties: typed request properties are available for
  supported constructs such as `text(@q)`, and conflicts between YQL controls
  and separately supplied typed controls are rejected. Arbitrary Vespa
  property namespaces and annotation semantics are not exposed. User-visible
  impact: property-dependent queries outside the supported binding surface do
  not run. Status: partial implementation.
- Query profiles and search chains: Search Kernel does not load or resolve the
  full Vespa query-profile or search-chain configuration system. Profile- or
  chain-dependent behavior is unavailable unless represented by an explicitly
  supported embedded configuration. User-visible impact: requests relying on
  those Vespa deployment controls are rejected or behave only according to
  the embedded configuration. Status: intentional embedded-vs-full-Vespa
  limitation.
- Embedded versus full Vespa behavior: Search Kernel embeds the pinned native
  search engine and exposes a deliberately smaller API. Configuration services,
  distributed routing, schema deployment, federation, and other container-
  level Vespa facilities are outside this wrapper. User-visible impact:
  behavior requiring a full Vespa cluster or container is not available in a
  local embedded engine. Status: intentional scope boundary.

## Acceptance and parity status

The checked-in M9 grouping verifier remains the frozen low-level recursive
grouping contract. Additional grouping/YQL acceptance coverage is maintained
separately so it does not weaken or rewrite M9. Current parity gaps are
tracked above until real pinned-Vespa execution proves the corresponding
portable and native paths. Verifier execution can also be blocked by host
tooling when bindings regeneration cannot find `libclang`; that is an
environment blocker, not a Vespa semantic limitation.
