# M8 source trace

Pinned Vespa source:

```text
vespa-engine/vespa@2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

`reference/vespa/PINNED_VESPA.json` must contain this same commit when the verifier is frozen.

The verifier author should confirm these paths/names against that exact revision before freezing the milestone.

## Local request/result

Primary pinned source areas:

```text
searchlib/src/vespa/searchlib/engine/searchrequest.h
searchlib/src/vespa/searchlib/engine/searchreply.h
searchlib/src/vespa/searchlib/engine/docsumrequest.h
searchlib/src/vespa/searchlib/engine/docsumreply.h
searchcore/src/vespa/searchcore/proton/server/documentdb.h
searchcore/src/vespa/searchcore/proton/server/documentdb.cpp
searchcore/src/vespa/searchcore/proton/matching/
```

The pinned `SearchRequest` exposes the local request fields relevant to M8:

```text
offset
maxhits
sortSpec
```

The pinned `SearchReply` exposes:

```text
totalHitCount
hits
hit metric/rank
sortIndex/sortData
```

The pinned `DocumentDB` exposes the local matching/docsums entry points used by the oracle and adapter.

## Sorting

Trace the pinned sort-spec parser/executor from `SearchRequest.sortSpec` into the local match post-processing path. The M8 contract depends only on Integer attributes; M9 broadens this.

## Docsums

Trace:

```text
DocumentDB::getDocsums
search::engine::DocsumRequest
search::engine::DocsumReply
summary configuration/document store path
```

The M8 oracle must include one indexed+summary Text field that is not an attribute.

## M7 composition

Query lowering remains owned by M7. M8 should reuse M7's native query representation rather than introducing a second query compiler.
