# M9 — Attributes, grouping, and facets

## Goal

Broaden the attribute capability first proven in M2 and compose it with the M8 local request/result path.

M9 exposes real Vespa attribute mutation, typed attribute matching/sorting/ranking access, and a small local grouping/aggregation surface.

```text
mutable attributes
      ↓
real Vespa attribute vectors
      ├── query filtering
      ├── sorting
      ├── ranking feature access
      └── local grouping / aggregation
```

Search Kernel does not implement a Rust facet or aggregation engine.

## Frozen API

The public Rust shape is frozen by:

```text
reference/vespa/api-contract/src/bin/m9.rs
```

M9 adds:

```text
AttributeDocument
AttributeUpdate
AttributeBound
Query::AttributeEquals
Query::AttributeRange
Aggregation
AggregateValue
LocalGroupingRequest
LocalGroup
LocalGroupingResult
Engine::put_attributes
Engine::update_attributes
Engine::group_local
```

M8 request/result types remain the local search surface.

## Supported attribute types

M9 v1 requires mutable **single-value** M6 attributes:

```text
String
Integer
Float
Boolean
```

Text indexing/update continues through M3/M4. M9 does not define a general all-field document builder.

`Value` type matching is exact:

```text
FieldType::String  <-> Value::String
FieldType::Integer <-> Value::Integer
FieldType::Float   <-> Value::Float
FieldType::Boolean <-> Value::Boolean
```

Float values must be finite.

Unknown fields, non-attribute fields, and wrong value types are errors before mutation is accepted.

## Attribute documents and updates

`AttributeDocument` creates a document containing one or more attribute values.

`AttributeUpdate` applies one or more assignment updates to an existing document.

Updates must travel through Vespa's real document-update/attribute path and become visible to subsequent queries, sorting, ranking, and grouping.

Rust must not retain an authoritative shadow map of attribute values.

An `AttributeDocument` with no fields and an `AttributeUpdate` with no assignments are invalid requests. Duplicate field names within one builder are outside M9 v1 and are not part of the frozen contract; verifier inputs never rely on duplicate assignments. Persisted/current values remain owned by Vespa.

## Structured filtering

M7's integer operators remain source-compatible.

M9 adds:

```text
Query::AttributeEquals {
    field,
    value,
}

Query::AttributeRange {
    field,
    lower: AttributeBound,
    upper: AttributeBound,
}
```

### AttributeEquals

Supported:

```text
String
Integer
Float
Boolean
```

The value type must exactly match the declared M6 attribute type.

### AttributeRange

Supported:

```text
Integer
Float
```

Bounds may be unbounded/included/excluded. Bounded values must match the field type. Float bounds must be finite.

Range on String or Boolean is an error in M9 v1.

### Presence / Exists

M7 `Query::Exists` retains its frozen M7 semantics for **Integer** attributes. M9 does not invent a generic existence operator for String or Boolean fields, because the pinned Vespa query model does not provide a general unset/null predicate for those scalar types.

For Float attributes, an `AttributeRange` with both bounds unbounded is the supported numeric presence query and matches documents where the Float attribute has a value. The verifier includes documents with unset numeric attributes to freeze this behavior.

No Rust-side post-filter is allowed.

## Sorting

After M9, the M8 `SortField` request surface supports:

```text
String attribute
Integer attribute
Float attribute
```

Boolean sorting remains outside M9 v1.

Sorting must use Vespa's native sort path and current attribute values.

## Ranking access

M9 proves that mutable numeric attributes are available to the real native ranking system.

M9 does **not** add arbitrary ranking configuration/expression APIs; M10 owns those.

The verifier has an immutable fixture rank profile:

```text
attribute_price
first-phase: attribute(price)
```

The ordinary M8 `rank_profile("attribute_price")` request is used before and after an attribute update. Returned real Vespa scores/order must change accordingly.

Rust must not read `price` and manufacture a score.

## Local grouping / facets

M9 v1 exposes one local grouping level:

```text
group by one attribute
  -> count
  -> optional numeric aggregations
```

Supported group keys:

```text
String attribute
Integer attribute
```

Float and Boolean grouping are not required in M9 v1.

A facet is represented by a `LocalGroupingRequest` with no explicit aggregation; `count` is always collected.

### Aggregations

Supported:

```text
Sum(field)
Min(field)
Max(field)
Average(field)
```

Aggregation fields must be numeric M6 attributes:

```text
Integer
Float
```

Aggregate result typing is frozen as:

```text
Sum/Min/Max(Integer attribute) -> AggregateValue::Integer(i64)
Average(Integer attribute)     -> AggregateValue::Float(f64)
Sum/Min/Max/Average(Float)     -> AggregateValue::Float(f64)
```

This preserves wide integer sums while keeping averages non-truncating.

Aggregation result order exactly follows request aggregation order. Duplicate aggregation requests are outside M9 v1 and are not part of the frozen contract.

### Query interaction

Grouping is over the documents matched by the request's `Query`.

The verifier includes a filtered facet scenario to prove the M7/M9 query path feeds grouping.

### No regular-hit dependency

`Engine::group_local` must execute with zero requested regular hits. Group output must still be complete.

This is a normative anti-shortcut requirement: an implementation that fetches regular hits and computes groups/counts/aggregates in Rust cannot satisfy the verifier.

### Group ordering

M9 does not freeze incidental local group iteration order.

Verifier output is canonicalized by typed group key before comparison.

M9 does not expose:

```text
group ordering expressions
group max/precision
continuations
nested grouping
group hits/summaries
distributed grouping merge
```

## Required native behavior

The implementation must use the pinned Vespa C++ attribute/grouping machinery corresponding to:

```text
real Document/DocumentUpdate attribute mutation
attribute-backed query nodes
SearchRequest.sortSpec
real rank profile evaluation reading attributes
SearchRequest.groupSpec / SearchReply.groupResult
searchlib grouping/aggregation execution
```

The exact internal classes may vary with the pinned revision; the behavior above is frozen.

## Required scenarios

The M9 corpus contains:

```text
01 String/Integer/Float/Boolean attribute insertion + exact filtering
02 integer and float range filtering
03 String/Boolean assignment update changes filter membership and native sort order
04 String attribute sorting + pagination
05 Float attribute sorting + pagination
06 native rank profile reads price; update changes score/order
07 String facet counts with zero regular hits
08 filtered facet counts
09 Integer group key + count
10 Integer sum/min/max/average by group, with sum exceeding i32
11 Float sum/min/max/average by group
12 update changes group key and aggregate values
13 Integer Exists + full unbounded Float range exclude unset attributes
```

## Required negative cases

```text
empty attribute document
empty attribute update
unknown attribute field in document
non-attribute field in document
wrong mutation value type
unknown/non-attribute/wrong-type AttributeUpdate assignment
non-finite Float AttributeUpdate assignment
unknown field in AttributeEquals
wrong AttributeEquals type
AttributeRange on String
AttributeRange on Boolean
mismatched range bound type
non-finite Float mutation/query value
sort on Boolean
sort on non-attribute field
group by unknown field
group by non-attribute field
group by unsupported Float or Boolean field
aggregate unknown field
aggregate non-attribute field
aggregate non-numeric field
```

## Oracle

M9 behavior is frozen from a direct pinned Vespa C++ oracle.

The oracle must:

- construct real multi-attribute documents and updates;
- execute real attribute-backed matching and sorting;
- use the frozen native `attribute(price)` rank profile;
- execute grouping inside the local Vespa engine;
- decode the local group result into canonical typed groups/aggregates;
- use zero regular hits for grouping scenarios;
- never compute grouping/aggregation in the oracle driver outside Vespa itself;
- not link Search Kernel production code.

See `oracle/ORACLE_SPEC.md`.

## Forbidden shortcuts

Production must not:

```text
retain a Rust shadow attribute store as authority
filter attributes in Rust after matching
sort hits in Rust
calculate ranking scores in Rust
calculate facet/group counts in Rust
calculate sum/min/max/avg in Rust
request all hits and aggregate them in Rust
read reference/vespa/m9 fixtures/goldens
invoke Java/Maven/JVM at runtime
call an external Vespa service
import searchkernel-core
```

## Definition of done

M9 is complete when:

```text
M0-M8 remain green
frozen m9.rs compiles
all four mutable attribute scalar types work
updates affect filtering/sorting/ranking/grouping
String/Integer/Float sorting uses native Vespa
native attribute rank access is proven
String/Integer group counts work with zero regular hits
numeric sum/min/max/avg match direct C++ Vespa
filtered facets compose with structured query matching
negative field/type/group/aggregation validation is frozen
no Rust aggregation/facet/ranking engine exists
no distributed grouping merge exists
verify-m9.sh passes
```
