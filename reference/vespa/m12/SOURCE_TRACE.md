# M12 pinned Vespa source trace

The oracle and adapter behavior in this milestone is tied to Vespa revision
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`, recorded in
`reference/vespa/PINNED_VESPA.json`. The paths below are repository-relative
paths in that checkout; they are intentionally source locations rather than
links to an unpinned Vespa branch.

| behavior | pinned source and symbol | wrapper use |
| --- | --- | --- |
| tensor document values | `document/src/vespa/document/fieldvalue/tensorfieldvalue.h`, `document/src/vespa/document/datatype/tensordatatype.h` | `sk_document_add_tensor_dense` constructs a real `TensorFieldValue` with the requested `TensorDataType` |
| tensor attribute declaration | `searchlib/src/vespa/searchlib/attribute/config.h`; `searchlib/src/vespa/searchlib/tensor/tensor_attribute.h` | generated `attributes.cfg` declares the tensor attribute; tensor matching stays in Vespa |
| distance metric | `searchlib/src/vespa/searchlib/tensor/distance_metric.h`; `searchcore/src/vespa/searchcore/proton/attribute/attribute_manager.cpp` | native configuration uses Vespa's tensor attribute distance metric |
| query tensor input | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` (`add_nearest_neighbor_term`); `searchlib/src/vespa/searchlib/query/tree/termnodes.h` (`NearestNeighborTerm`) | query tensor names and target/approximate controls reach the native query builder |
| HNSW execution parameters | `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.h` (`NearestNeighborBlueprint::HnswParams`); `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.cpp` | exposed and deferred controls are frozen in `LIMITATIONS.md` |
| nearest-neighbor matching | `searchcore/src/vespa/searchcore/proton/matching/nearest_neighbor_matcher.cpp`; `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.cpp` | `sk_engine_search` executes the real Proton/QueryEval path |
| query stack encoding | `searchlib/src/vespa/searchlib/query/tree/querybuilder.cpp`; `searchlib/src/vespa/searchlib/query/stackdump.cpp` | query construction remains inside the native adapter |

The Phase 9 additions have their own trace in
`reference/vespa/phase9/SOURCE_TRACE.md`. This file is the frozen M12 trace,
not an implementation checklist.
