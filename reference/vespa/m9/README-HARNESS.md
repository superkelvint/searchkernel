# M9 verifier harness

M9 broadens the M2/M8 attribute path and exposes a deliberately small projection of Vespa's **local** grouping/aggregation capability. It is cumulative on M8.

## Files

```text
CONTRACT.md                 normative M9 behavior and non-goals
SOURCE_TRACE.md             pinned Vespa attribute/grouping source areas
scenarios/*.json            frozen positive/negative scenario corpus
fixture/                    verifier-owned rank/config fixture material
oracle/ORACLE_SPEC.md       direct C++ oracle behavior
oracle/AUTHOR_ORACLE.md     exact authoring sequence
oracle/build-cpp-oracle.sh  M4-style direct-C++ build/provenance wrapper
oracle/run-cpp-oracle.sh    provenance-checked oracle runner
expected/*.json             direct-oracle goldens; absent until freeze
rust-driver/                implementation-side scenario driver
canonicalize.py             canonicalizes only non-contracted group order
validate_scenarios.py       guards update/grouping anti-shortcut traps
validate_expected.py        validates canonical output shape
compare.py                  canonical semantic comparison
check-structure.sh          defense-in-depth native-path checks
freeze-goldens.sh           runs a reviewed oracle; never compiles it
freeze-immutable.sh         freezes the verifier
check-immutable.sh          implementation-time immutability check
verify-m9.sh                cumulative acceptance command
```

The public API is frozen by `reference/vespa/api-contract/src/bin/m9.rs`.

## Authoring order

1. Complete and freeze M8 first.
2. Review the M9 API/contract/scenario corpus.
3. Prepare one verifier-owned M9 engine configuration containing the four mutable scalar attributes (`category`, `qty`, `price`, `active`) plus the immutable `attribute_price` ranking profile. The direct oracle must not obtain configuration by calling the M9 implementation.
4. Author `oracle/m9_attribute_grouping_oracle.cpp` by extending the already-proven local `DocumentDB`/persistence harness.
5. Use real Vespa `Document` and `DocumentUpdate` values for mutation.
6. Use real native attribute query/sort/ranking paths.
7. Construct/serialize one-level grouping through the pinned Vespa grouping objects. A verifier-only Java helper is allowed only if the pinned request serialization genuinely resides in Java; if used, freeze the resulting grouping bytes and keep actual execution in direct C++.
8. Every grouping request must request zero regular hits. The direct oracle must assert the actual ordinary hit vector is empty while native `groupResult` remains populated.
9. Build/review the oracle, then run:

   ```bash
   ./reference/vespa/m9/freeze-goldens.sh
   ./reference/vespa/m9/freeze-immutable.sh
   ```

10. Hand M9 to implementation only after `M9-IMMUTABLE.sha256` exists.

## Critical traps

- all four scalar attribute types are inserted;
- String/Boolean, Float, and Integer assignment updates are all exercised across the corpus;
- a String update is subsequently observed by both filtering **and native sorting**;
- a Float update changes real `attribute(price)` rank scores/order;
- an Integer update changes native grouping/aggregation output;
- String and Float sorting are exercised separately;
- the main facet fixture contains 25 documents, exceeding the normal ten-hit page;
- grouping requests have zero regular hits, so result-page aggregation cannot pass;
- filtered facets prove the query matcher feeds grouping;
- `composite-attribute-grouping` carries M7 boolean lowering through attribute filtering and native grouping;
- integer and floating point sum/min/max/average are taken from native group results; the integer-sum fixture exceeds `i32::MAX` to prove the `i64` result contract;
- M7 Integer `Exists` remains unchanged; Float presence uses an all-unbounded numeric range. No generic String/Boolean existence API is invented.

## What M9 explicitly does not add

```text
nested grouping
group ordering API
group max/precision
continuations
group hit collection/group summaries
distributed grouping merge
Boolean sort
Boolean group keys
multi-value attributes
arbitrary rank-profile/ranking-expression API
Rust aggregation/facet engine
```
