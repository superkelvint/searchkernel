# Phase 9 pinned Vespa source trace

All paths below are from pinned revision
`2300ef0412bdb76f227ae3b497c8ca5e73c5da4e`. The source trace is part of the
acceptance boundary for the adapter; implementation must not infer these
semantics from symbol names alone.

| capability | pinned source | adapter boundary |
| --- | --- | --- |
| NN options | `searchlib/src/vespa/searchlib/queryeval/nearest_neighbor_blueprint.h` (`NearestNeighborBlueprint::HnswParams`); `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` (`add_nearest_neighbor_term`) | options handle sets `distance_threshold` and `explore_additional_hits`; deferred fields are frozen in `m12/LIMITATIONS.md` |
| grouping controls | `searchlib/src/vespa/searchlib/aggregation/groupinglevel.h` (`setMaxGroups`, `setPrecision`, `addOrderBy`) | native grouping level setters and order-by handle |
| grouping filter | `searchlib/src/vespa/searchlib/aggregation/groupinglevel.h` (`Filter = expression::FilterPredicateNode`); `searchlib/src/vespa/searchlib/aggregation/groupinglevel.cpp` (`serializeVariant`, `deserializeVariant`) | adapter installs a pinned `GroupingLevelV2` round-trip so the filter is retained without exposing a Vespa type to Rust |
| scalar aggregation results | `searchlib/src/vespa/searchlib/aggregation/{sum,min,max,standarddeviation,xor}aggregationresult.*` | typed aggregation constructors and result conversion |
| quantiles | `searchlib/src/vespa/searchlib/aggregation/quantile_aggregation_result.h` (`quantile_results`) | native quantile accessor returns `(quantile,value)` pairs |
| summary/hits | `searchlib/src/vespa/searchlib/aggregation/hitsaggregationresult.h` (`getResult`, `visitMembers`); `searchlib/src/vespa/searchlib/aggregation/hitlist.cpp` (`HitList::visitMembers`) | native `HIT_LIST` output copies document IDs and Rust constructs `GroupingOutput::HitList` |
| arithmetic update | `document/src/vespa/document/update/arithmeticvalueupdate.h/.cpp` (`ArithmeticValueUpdate::applyTo`); `document/src/vespa/document/update/fieldupdate.h` | adapter creates a real `FieldUpdate` containing `ArithmeticValueUpdate` |
| raw sort | `searchlib/src/tests/sort/sort_test.cpp`; `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` | raw sort is an explicit safe Rust escape hatch, mutually exclusive with typed sort fields |
| `in` query | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` (`add_in_term`); `searchlib/src/vespa/searchlib/query/tree/termnodes.h` (`InTerm`) | native string/integer constructors are exercised by the matcher verifier |
| sameElement | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h`; `searchlib/src/vespa/searchlib/query/tree/querybuilder.cpp`; `searchcore/src/vespa/searchcore/proton/server/storeonlyfeedview.cpp` | native constructor and portable array/object lowering are exercised for array strings and relative object-array members; the adapter publishes the supported member fields and verifies `elementFilter` through the real matcher |
| geo | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h`; `searchlib/src/vespa/searchlib/query/tree/querybuilder.cpp`; `vespalib/src/vespa/vespalib/geo/zcurve.h` | native position/geo lowering is exercised with the adapter's Vespa-compatible derived position zcurve and real inside/outside/boundary documents |
| predicate | `searchlib/src/vespa/searchlib/query/tree/querybuilder.h` (`addPredicateQuery`); `searchlib/src/vespa/searchlib/query/tree/predicate_query_term.h`; `searchlib/src/vespa/searchlib/predicate/predicate_range_expander.h`; `document/src/vespa/document/predicate/predicate.h` | legacy features and typed range queries use native predicate terms; canonical Slime documents are parsed and range nodes receive Vespa-native partition/edge annotations in the C++ adapter before indexing |

The grouping-filter verifier asserts that the serialized variant contains the
V2 class ID and survives deserialization. If that pinned wire layout changes,
the verifier must fail and the adapter must be re-audited.

The same-element acceptance is intentionally limited to the array and relative
member field shapes represented by the Search Kernel document model. It does
not claim all Vespa struct/map or arbitrary dotted-field forms.
