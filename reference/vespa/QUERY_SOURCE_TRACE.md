# Typed query source trace

The query semantics in this verifier are pinned to the Vespa checkout named by
`reference/vespa/PINNED_VESPA.json`.

`upstream/vespa/searchlib/src/vespa/searchlib/query/tree/querybuilder.h`
provides the native query-tree boundary used by the adapter:

- `QueryBuilder::add_in_term()` creates Vespa's `InTerm` node.
- `QueryBuilder::addWeakAnd()` creates the default weak-and text operator.
- `QueryBuilder::addWandTerm()` accepts target hit count, score threshold, and
  threshold boost factor.
- `QueryBuilder::add_nearest_neighbor_term()` accepts target hits, approximate
  mode, and `NearestNeighborTerm::HnswParams`.

The Search Kernel mapping is therefore:

```text
typed In       -> native InTerm
typed text     -> analyzed terms under native WeakAnd
typed phrase   -> native Phrase
typed WAND     -> native WAND controls when annotated
typed NN       -> native NN controls when annotated
```

`userInput`, `userQuery`, and `NonEmpty` remain explicit unsupported cases in
the typed adapter. Same-element, geo, and predicate query nodes have native
tree plumbing, but their required array/struct, position, and predicate schema
and document paths are not yet part of the Search Kernel product contract.
Those limitations are recorded in `LIMITATIONS.md`.
