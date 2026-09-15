# M9 source trace

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

`reference/vespa/PINNED_VESPA.json` must contain this same commit when the verifier is frozen.

The verifier author must resolve exact classes/header paths against that checkout before freezing.

## Attribute mutation/access

Relevant source areas include:

```text
document/src/vespa/document/fieldvalue/
document/src/vespa/document/update/
searchlib/src/vespa/searchlib/attribute/
searchcore/src/vespa/searchcore/proton/attribute/
searchcore/src/vespa/searchcore/proton/server/documentdb.*
searchcore/src/vespa/searchcore/proton/persistenceengine/
```

Trace the real put/update path from Document/DocumentUpdate into attribute vectors and publication to subsequent search.

## Attribute matching/sorting/ranking

Relevant areas:

```text
searchlib/src/vespa/searchlib/query/tree/
searchcore/src/vespa/searchcore/proton/matching/
searchlib/src/vespa/searchlib/attribute/
searchlib/src/vespa/searchlib/fef/
```

M9 must prove that current attribute values feed matching, sort data, and a frozen native `attribute(price)` rank profile.

## Grouping/aggregation

Relevant source areas:

```text
searchlib/src/vespa/searchlib/aggregation/
searchlib/src/vespa/searchlib/expression/
searchlib/src/vespa/searchlib/grouping/
searchcore/src/vespa/searchcore/proton/matching/
searchlib/src/vespa/searchlib/engine/searchrequest.h
searchlib/src/vespa/searchlib/engine/searchreply.h
```

Trace the pinned serialization/execution path connecting:

```text
SearchRequest.groupSpec
  -> local grouping execution
  -> SearchReply.groupResult
```

The direct C++ oracle is authoritative for execution. If the verifier-authoring implementation uses pinned Java classes to serialize a grouping specification, record those Java source paths too and keep that helper verifier-only.

## Upstream behavioral references

Vespa's grouping semantics establish that grouping operates over the complete query match set, grouping fields are attributes, and `count`, `sum`, `min`, `max`, and `avg` are native aggregators. M9 deliberately freezes only a one-level local subset and does not reproduce distributed merge semantics.
