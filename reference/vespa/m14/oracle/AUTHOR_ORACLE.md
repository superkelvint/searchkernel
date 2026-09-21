# Authoring/reviewing the M14 direct oracle

`m14_hnsw_oracle.cpp` is verifier code, not production code. Review it against
`SOURCE_TRACE.md` and the pinned Vespa revision before freezing expected data.

The review must confirm that it:

1. constructs a real Proton `DocumentDB`;
2. configures a tensor attribute with `distancemetric EUCLIDEAN`;
3. sets `index.hnsw.enabled true`, max-links 4, neighbors-at-insert 16;
4. feeds real `TensorFieldValue` documents through persistence;
5. passes the requested exact/approximate bit and HNSW params directly to
   `QueryBuilder::add_nearest_neighbor_term`;
6. uses real persistence update/remove operations for graph maintenance;
7. reopens the same data directory for persistence checks;
8. contains no Search Kernel production include or linkage.

After review:

```bash
reference/vespa/m14/oracle/build-cpp-oracle.sh
ALLOW_M14_GOLDEN_REGEN=1 reference/vespa/m14/freeze-goldens.sh
```

Commit the reviewed verifier/goldens before implementation begins.
