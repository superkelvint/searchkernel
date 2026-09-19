# Recursive grouping source trace

Pinned Vespa revision is whatever is recorded by:

```text
reference/vespa/PINNED_VESPA.json
```

This verifier reuses the M9 direct-DocumentDB oracle harness and fixture. The
following exact pinned files are the source trace for the capability:

```text
searchlib/src/vespa/searchlib/aggregation/group.h
searchlib/src/vespa/searchlib/aggregation/group.cpp
searchlib/src/vespa/searchlib/aggregation/grouping.h
searchlib/src/vespa/searchlib/aggregation/grouping.cpp
searchlib/src/vespa/searchlib/aggregation/groupinglevel.h
searchlib/src/vespa/searchlib/aggregation/groupinglevel.cpp
searchlib/src/vespa/searchlib/aggregation/aggregationresult.h
searchlib/src/vespa/searchlib/aggregation/hitsaggregationresult.h
searchlib/src/vespa/searchlib/aggregation/hitsaggregationresult.cpp
searchlib/src/vespa/searchlib/aggregation/countaggregationresult.h
searchlib/src/vespa/searchlib/aggregation/countaggregationresult.cpp
searchlib/src/vespa/searchlib/aggregation/averageaggregationresult.h
searchlib/src/vespa/searchlib/aggregation/averageaggregationresult.cpp
searchlib/src/vespa/searchlib/expression/expressionnode.h
searchlib/src/vespa/searchlib/expression/attributenode.h
searchlib/src/vespa/searchlib/expression/attributenode.cpp
searchlib/src/vespa/searchlib/expression/resultnode.h
searchlib/src/vespa/searchlib/expression/resultnode.cpp
searchlib/src/vespa/searchlib/engine/searchrequest.h
searchlib/src/vespa/searchlib/engine/searchreply.h
searchcore/src/vespa/searchcore/proton/matching/matcher.h
searchcore/src/vespa/searchcore/proton/matching/matcher.cpp
```

These sources cover `Grouping`, `GroupingLevel`, `Group`, the
`AggregationResult` hierarchy, `HitsAggregationResult`, and the expression
node/classification machinery used by attribute classifiers and result nodes.
The oracle constructs two real `GroupingLevel` values and serializes the
resulting `Grouping` into `SearchRequest.groupSpec`. It then executes through
the pinned embedded `DocumentDB` matcher and deserializes
`SearchReply.groupResult`.

The Search Kernel implementation under test must reach the same Vespa execution
path from the private C ABI.
