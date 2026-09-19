# Recursive grouping ABI verifier

This is intentionally smaller than a full "general grouping" milestone.

It freezes exactly one new low-level capability:

```text
sk_grouping_plan
    └── levels[]

category
  -> qty
```

with `count()` and `avg(price)` at both grouping levels, traversed recursively
through the private C ABI. The plan is a linear searchlib-aligned backend
shape. Higher-level `all()` and `each()` grouping containers are intentionally
not native ABI nodes; branching higher-level grouping may compile to multiple
backend plans. Each level is classified by an opaque
`sk_grouping_expression`; this milestone supports only typed attribute
expressions for `category` and `qty`. Aggregations use domain-specific opaque
handles for `count()` and `average(expression)`, attached to the level and
destroyed by the caller after attachment.

Results remain recursive. Keys use grouping-specific result values, and node
outputs use a separate grouping-output classification (`VALUE` now,
`HIT_LIST` reserved for later summary support). This is not the universal
`sk_value` domain and is not flattened into facets.

It does **not** cover root aggregations, aliases, order, precision, keep/filter,
summary hits, continuations, stddev, IR lowering, or Python.

It intentionally reuses:

```text
reference/vespa/m9/fixture/
reference/vespa/m9/oracle/m9_attribute_grouping_oracle.cpp
reference/vespa/common/verify-common.sh
reference/vespa/common/compare_json.py
```

## Workflow

The expected output is frozen from the direct pinned-Vespa oracle before native
implementation:

```bash
reference/vespa/m9-general-grouping-verifier/oracle/build-cpp-oracle.sh
reference/vespa/m9-general-grouping-verifier/freeze-golden.sh
```

Review and commit the verifier, source trace, scenario, and golden.

The verifier compiles a frozen C API fixture and compares both runtime paths:

```bash
reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh
```

passes. The structure checks require the new native path to use Vespa grouping
classes and reject fixture-specific shortcuts, repeated searches, and
C-series IR coupling.
