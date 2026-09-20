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
- `userInput(...)` and `userQuery(...)` implement the pinned adapter's small
  parser contract: weak-and terms, quoted phrases, leading negative terms,
  field selection, parentheses, and explicit `AND`/`OR`. Vespa's complete
  configurable user-input grammar, escaping rules, and parser modes are not
  implemented.
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
  `array<string>`, position values, and feature-only predicate values. It does
  not yet support arbitrary arrays, `array<struct>`, nested structs, predicate
  range values, or SameElement element filters.
- `sameElement`, geo position queries, and predicate queries have native
  query-tree plumbing. Predicate has a real schema/document/search execution
  proof; SameElement and geo remain limited by the small schema/indexing
  contract and are not yet frozen as full Vespa-compatible scenarios.
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
- The wrapper does not expose Vespa's complete result tree, grouping, summary,
  ranking-expression, or total-hit relation surface. Unsupported portions are
  rejected or represented only by the frozen Search Kernel contract.
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
