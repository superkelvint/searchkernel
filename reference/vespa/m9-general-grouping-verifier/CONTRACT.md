# M9 general grouping — grouping-plan ABI core

## Goal

Prove the first general-grouping capability at the private Search Kernel native ABI boundary.

This verifier intentionally covers one narrow semantic slice:

```text
query matching all fixture documents
  -> group by category
       -> count()
       -> avg(price)
       -> subgroup by qty
            -> count()
            -> avg(price)
```

The result must be returned as a recursive tree through the C ABI.

The frozen data set is intentionally small:

```text
category=A, qty=1, price=10
category=A, qty=1, price=20
category=A, qty=2, price=30
category=B, qty=1, price=40
```

The query matches `0 <= price <= 100` and requests zero ordinary hits.

This is an extension of the low-level M9 grouping capability. It is **not** a
portable/client FFI and it does not involve `searchkernel-ir`, the dispatcher,
Python, gRPC, or `searchkerneld`.

## Required execution path

```text
C verifier
  -> Search Kernel private native ABI
  -> C++ Search Kernel Vespa adapter
  -> real Vespa local grouping execution
  -> recursive native result tree
  -> C verifier
```

The direct pinned-Vespa C++ oracle executes the same scenario without calling
the Search Kernel grouping ABI.

## Frozen native ABI surface

The implementation must add opaque handle declarations equivalent to:

```c
typedef struct sk_grouping_plan sk_grouping_plan;
typedef struct sk_grouping_level sk_grouping_level;
typedef struct sk_grouping_expression sk_grouping_expression;
typedef struct sk_grouping_aggregation sk_grouping_aggregation;
typedef struct sk_grouping_result_value sk_grouping_result_value;
typedef struct sk_grouping_tree_output sk_grouping_tree_output;
typedef struct sk_grouping_tree_result sk_grouping_tree_result;
typedef struct sk_grouping_tree_node sk_grouping_tree_node;

#define SK_GROUPING_RESULT_VALUE_STRING 1U
#define SK_GROUPING_RESULT_VALUE_INTEGER 2U
#define SK_GROUPING_RESULT_VALUE_FLOAT 3U
#define SK_GROUPING_TREE_OUTPUT_VALUE 1U
#define SK_GROUPING_TREE_OUTPUT_HIT_LIST 2U
```

and the following functions to the public private-native header
`core/native/vespa/searchkernel_vespa.h`:

```c
#include <stdint.h>

int32_t sk_grouping_plan_create(
    sk_grouping_plan **out_plan,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_expression_attribute(
    const char *field,
    sk_grouping_expression **out_expression,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_grouping_expression_destroy(
    sk_grouping_expression *expression);

int32_t sk_grouping_aggregation_count(
    sk_grouping_aggregation **out,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_aggregation_average(
    const sk_grouping_expression *expression,
    sk_grouping_aggregation **out,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_grouping_aggregation_destroy(
    sk_grouping_aggregation *aggregation);

/* Clones the classifier's semantic expression. The caller may destroy
 * classifier immediately after a successful call. */
int32_t sk_grouping_plan_add_level(
    sk_grouping_plan *plan,
    const sk_grouping_expression *classifier,
    sk_grouping_level **out_level,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_level_add_count(
    sk_grouping_level *level,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_level_add_aggregation(
    sk_grouping_level *level,
    uint32_t kind,
    const char *field,
    uint8_t *out_error,
    uint32_t out_error_capacity);

/* Canonical typed aggregation attachment. The level clones the aggregation's
 * semantic value; the caller may destroy aggregation after success. */
int32_t sk_grouping_level_add_aggregation_handle(
    sk_grouping_level *level,
    const sk_grouping_aggregation *aggregation,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_grouping_plan_destroy(sk_grouping_plan *plan);

int32_t sk_engine_group_tree(
    const sk_engine *engine,
    const sk_query *query,
    const sk_grouping_plan *plan,
    sk_grouping_tree_result **out_result,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_tree_result_root(
    const sk_grouping_tree_result *result,
    const sk_grouping_tree_node **out_root,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_grouping_tree_node_group_count(
    const sk_grouping_tree_node *node);

int32_t sk_grouping_tree_node_group(
    const sk_grouping_tree_node *node,
    uint32_t group_index,
    const sk_grouping_tree_node **out_group,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_tree_node_key(
    const sk_grouping_tree_node *node,
    const sk_grouping_result_value **out_value,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_grouping_result_value_kind(
    const sk_grouping_result_value *value);

int32_t sk_grouping_result_value_string(
    const sk_grouping_result_value *value,
    const char **out_string,
    uint32_t *out_length,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_result_value_integer(
    const sk_grouping_result_value *value,
    int64_t *out_integer,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_grouping_result_value_float(
    const sk_grouping_result_value *value,
    double *out_float,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_grouping_tree_node_output_count(
    const sk_grouping_tree_node *node);

int32_t sk_grouping_tree_node_output(
    const sk_grouping_tree_node *node,
    uint32_t output_index,
    const sk_grouping_tree_output **out_output,
    uint8_t *out_error,
    uint32_t out_error_capacity);

uint32_t sk_grouping_tree_output_kind(
    const sk_grouping_tree_output *output);

int32_t sk_grouping_tree_output_value(
    const sk_grouping_tree_output *output,
    const sk_grouping_result_value **out_value,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_grouping_tree_result_destroy(sk_grouping_tree_result *result);
```

### Backend plan shape

The request-side ABI is deliberately a searchlib-aligned grouping plan:

```text
sk_grouping_plan
    └── levels[]
```

`sk_grouping_plan_add_level` appends one grouping level to that ordered list.
Each level contains its classifier and that level's aggregations. The plan is a
linear nesting path; it is not a general grouping operation tree.

The first milestone freezes only the typed attribute constructor:

```text
attribute("category")
attribute("qty")
```

The expression handle is opaque. The ABI does not expose an expression
language or a generic expression-property map. Future expression families are
added as separate typed constructors, such as literal, function, unary,
binary, bucket, map lookup, member access, or alias expressions. These are
additive constructors and do not change the attribute constructor or the plan
level signature.

Grouping aggregations are likewise domain-specific opaque handles. This
milestone freezes typed `count` and `average` constructors only. An average
constructor clones the supplied classifier expression into the aggregation
handle, so the caller may destroy that expression after successful
construction. A level clones the aggregation semantic value when the handle is
attached.

The existing `sk_grouping_level_add_count` and
`sk_grouping_level_add_aggregation(kind, field, ...)` functions remain
permanently available as convenience wrappers. Their signatures and semantics
are not changed or removed. They must construct the same underlying
domain-specific grouping representation as the typed-handle path. Because the
C ABI cannot overload `sk_grouping_level_add_aggregation`, the canonical
typed-handle attachment function is named
`sk_grouping_level_add_aggregation_handle`.

The Java/container-style `all()` and `each()` constructs are not native ABI
nodes and must not acquire corresponding `sk_grouping_all` or
`sk_grouping_each` handles. They belong to Search Kernel's higher-level
grouping representation. Its compiler may lower branching grouping syntax
into however many backend `sk_grouping_plan` values and native executions are
needed. The low-level ABI remains aligned with the actual searchlib grouping
plan rather than modeling the source-language container syntax.

### Ownership

- `sk_grouping_plan` owns every `sk_grouping_level` returned by
  `sk_grouping_plan_add_level`.
- `sk_grouping_expression_attribute` returns a caller-owned expression.
- `sk_grouping_plan_add_level` clones and owns the semantic expression it needs;
  it does not borrow the classifier after the call returns.
- After a successful `sk_grouping_plan_add_level`, the caller may immediately
  destroy the classifier with `sk_grouping_expression_destroy`. On failure,
  ownership remains with the caller.
- `sk_grouping_level *` is a borrowed mutable handle from its parent plan and
  remains valid only while that plan is alive.
- Levels are appended in nesting order. For this verifier, level 0 is
  `category`, level 1 is `qty`.
- `sk_grouping_tree_result` owns all result nodes.
- `const sk_grouping_tree_node *` values are borrowed and valid until the
  result is destroyed.
- `sk_grouping_result_value` and `sk_grouping_tree_output` handles returned
  from a node are borrowed from the result and remain valid until the result is
  destroyed.
- String pointers returned by `sk_grouping_result_value_string` are borrowed
  from the result and remain valid until the result is destroyed.
- Destroy functions accept null.

No Vespa C++ types or C++ standard-library types may cross this ABI.

### Result representation

Grouping result values use the grouping-specific opaque
`sk_grouping_result_value` domain. This is not `sk_value` and must not be
implemented through a universal value system. M9 supports only
`STRING`, `INTEGER`, and `FLOAT`; additional grouping result kinds are added
with new kind constants and accessors.

Node keys are borrowed result values:

```text
sk_grouping_tree_node
        └── borrowed sk_grouping_result_value key
```

The value and any string storage returned from it remain valid until the
owning grouping result is destroyed.

Grouping node outputs are classified independently from their payload:

```text
sk_grouping_tree_output
    ├── VALUE    -> sk_grouping_result_value
    └── HIT_LIST
```

`HIT_LIST` is Search Kernel's ABI-level classification. Vespa represents this
using `HitsAggregationResult`, which is an `AggregationResult`; it is not a
Vespa class/type sibling of ordinary aggregation results. M9 implements only
`VALUE`, but the output handle exists now so later `summary()` support can add
the hit-list payload without redesigning recursive result traversal.

### Native Vespa mapping

The adapter must preserve the real Vespa/searchlib grouping split:

```text
sk_grouping_level
        ↓
search::aggregation::GroupingLevel

classifier
        ↓
GroupingLevel classification expression

max groups / precision / filter
        ↓
GroupingLevel state

aggregation + order expressions
        ↓
GroupingLevel's Group prototype
```

The implementation must execute through Vespa's grouping machinery. It must
not create a fake C++ aggregation evaluator or post-process flat results in
Rust into a pretend grouping tree.

## Request semantics frozen by this verifier

The request contains exactly two levels:

```text
level 0: group by category
         output count()
         output avg(price)

level 1: group by qty
         output count()
         output avg(price)
```

`sk_grouping_level_add_aggregation` must accept the existing
`SK_AGGREGATION_AVERAGE` kind for this verifier.

Output order is request order:

```text
0 -> count()
1 -> avg(price)
```

## Result semantics frozen by this verifier

The result has a root node. The root itself has no key and no outputs in this
slice. Its child groups are the first grouping level.

Every group node exposes:

```text
key
outputs[]
child groups[]
```

For this scenario:

```text
root
  category=A
    outputs: count=3, avg(price)=20
    qty=1
      outputs: count=2, avg(price)=15
    qty=2
      outputs: count=1, avg(price)=30

  category=B
    outputs: count=1, avg(price)=40
    qty=1
      outputs: count=1, avg(price)=40
```

Group iteration order is not frozen. The verifier canonicalizes groups by typed
key before comparison.

## Anti-shortcut requirements

The implementation must not:

- fetch ordinary hits and group them in Rust or C++;
- calculate count or average outside Vespa grouping execution;
- flatten the nested result;
- special-case the fixture field names or values;
- convert grouping into repeated searches;
- reconstruct nested grouping from separately executed queries;
- bypass the real Vespa grouping machinery;
- encode the request/result as JSON or portable FlatBuffers across the native ABI;
- depend on `searchkernel-ir`;
- modify the Python E2E;
- implement aliases, keep filters, ordering, precision, root outputs, stddev,
  summary hits, or continuation tokens as part of this slice.

The query must request zero ordinary result hits for grouping execution.

## Compatibility

The existing flat M9 grouping ABI must remain source/binary compatible in this
slice. It may internally delegate to the new implementation, but its symbols
must not be removed.

The verifier uses the repository's current native ABI version. The cumulative
ABI authorities and M0 acceptance fixture must remain consistent with the
version exported by the SDK.

## Acceptance

The verifier must first compile its frozen C fixture against the native SDK.
That fixture constructs attribute expressions, a grouping plan, two levels,
typed count/average aggregation handles, and recursively traverses borrowed
result handles. It must not rely on a Rust wrapper to prove the C ABI.

`verify-recursive-grouping.sh` must prove all three outputs are equal:

```text
frozen expected
      ==
direct pinned-Vespa C++ oracle
      ==
Search Kernel C ABI probe
```
