# M10 verifier harness

M10 verifies scalar local ranking and feature inspection against the pinned Vespa C++ engine. It is cumulative on M9.

## Files

```text
CONTRACT.md
SOURCE_TRACE.md
scenarios/*.json
expected/*.json
fixture/
oracle/ORACLE_SPEC.md
oracle/AUTHOR_ORACLE.md
oracle/m10_ranking_oracle.cpp
oracle/build-cpp-oracle.sh
oracle/run-cpp-oracle.sh
rust-driver/
validate_scenarios.py
validate_expected.py
config-regressions.json
validate_regressions.py
compare.py
canonicalize.py
check-structure.sh
freeze-goldens.sh
verify-m10.sh
```

The public Rust shape is additionally frozen by:

```text
reference/vespa/api-contract/src/bin/m10.rs
```

`config-regressions.json` contains focused engine-open checks for an
attribute-only schema with one custom profile and a text schema with a custom
profile named `bm25`. These checks are separate from the eight-result ranking
corpus.

## Freeze boundary

Authoring sequence:

```text
review contract/API/scenarios
        ↓
build direct pinned-Vespa C++ oracle
        ↓
run freeze-goldens.sh
        ↓
review exact score/feature bits
        ↓
commit verifier + expected outputs
        ↓
only then implement M10 production code
```

`verify-m10.sh` never regenerates expected output.

## Why exact floating-point bits

Ranking is the subject of this milestone. Approximate comparisons would permit an implementation to recompute scores in Rust and merely get close.

All scores and scalar feature values therefore use canonical IEEE-754 bit strings:

```text
0x3ff0000000000000
```

## Anti-shortcut traps

The corpus deliberately includes:

- BM25 values that cannot be inferred from an attribute;
- a query feature that changes score/order;
- the same input supplied as a raw native rank property;
- a pure arithmetic expression over a native attribute feature;
- `nativeRank` and its component features;
- multiple match features returned per hit.

Production code must not contain the scenario IDs, fixture paths, or expected score literals.
