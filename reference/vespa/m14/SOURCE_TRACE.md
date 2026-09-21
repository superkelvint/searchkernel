# M14 pinned Vespa source trace

M14 is tied to Vespa revision `2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`,
recorded in `reference/vespa/PINNED_VESPA.json`.

Paths below are repository-relative paths in that pinned checkout.

| behavior | pinned source and symbol | M14 use |
| --- | --- | --- |
| HNSW attribute configuration | `searchlib/src/vespa/searchlib/attribute/config.h` (`Config::HnswIndexParams`) | freezes enabled/max-links/neighbors-at-insert semantics emitted in `attributes.cfg` |
| tensor distance metric | `searchlib/src/vespa/searchlib/tensor/distance_metric.h`; `searchcore/src/vespa/searchcore/proton/attribute/attribute_manager.cpp` | freezes tensor attribute distance metric selection |
| HNSW tensor attribute | `searchlib/src/vespa/searchlib/tensor/hnsw_index.h`; `searchlib/src/vespa/searchlib/tensor/dense_tensor_attribute.h` | graph is owned and maintained by Vespa, not Search Kernel |
| HNSW index maintenance | `searchlib/src/vespa/searchlib/tensor/hnsw_index.cpp`; `searchlib/src/vespa/searchlib/tensor/hnsw_index_saver.cpp` / loader counterparts | insert/update/delete/reopen lifecycle remains Vespa behavior |
| NN query node | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` (`add_nearest_neighbor_term`); `searchlib/src/vespa/searchlib/query/tree/termnodes.h` (`NearestNeighborTerm`) | target hits, approximate flag and HNSW params enter the native query tree |
| ANN execution | `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.h`; `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.cpp` | selects HNSW top-k when approximate search is requested and an index is available |
| Proton NN matching | `searchcore/src/vespa/searchcore/proton/matching/nearest_neighbor_matcher.cpp` | real DocumentDB matcher executes exact/ANN queries |
| document tensor mutation | `document/src/vespa/document/fieldvalue/tensorfieldvalue.h`; `document/src/vespa/document/update/assignvalueupdate.h`; Proton persistence update/remove path | graph maintenance is verified after tensor assignment and deletion |

M12 remains the source trace for dense query tensor encoding and the existing
nearest-neighbor options ABI. M14 does not widen that ABI.
