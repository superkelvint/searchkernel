# Authoring the M9 oracle

Implement `m9_attribute_grouping_oracle.cpp` according to `ORACLE_SPEC.md` using the direct M8/M4 Vespa lifecycle.

Before coding, inspect the exact pinned aggregation/expression serialization path. Do not guess a grouping binary format.

Recommended workflow:

1. verify pinned Vespa revision;
2. prove multi-field attribute Document/DocumentUpdate mutation directly in the C++ oracle;
3. prove direct String/Integer/Float/Boolean matching;
4. freeze the `attribute_price` rank profile fixture and prove score comes from `Hit::metric`;
5. construct/serialize one-level grouping with native searchlib objects;
6. run at least one `maxhits=0` count-only grouping before adding more aggregators;
7. add sum/min/max/avg decoding;
8. run all positive scenarios in a single live engine where scenario steps require updates;
9. freeze goldens and manually inspect update-sensitive cases;
10. freeze immutable verifier only after independent review.

The implementation agent for M9 must not regenerate these goldens.
