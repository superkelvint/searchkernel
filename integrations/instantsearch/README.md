# InstantSearch integration

This directory is reserved for Search Kernel's InstantSearch integration.

The intended architecture is:

```text
InstantSearch.js / React InstantSearch / Vue InstantSearch
        ↓
Search Kernel InstantSearch adapter
        ↓
searchkerneld Search Endpoint
        ↓
canonical SearchRequest
        ↓
searchkernel-dispatch
        ↓
Search Kernel IR
        ↓
Vespa
```

The integration is an adapter for the InstantSearch search-client contract. It is **not** a second Search Kernel client and must not implement independent search semantics.

## Intended responsibilities

The adapter may translate:

- InstantSearch requests into Search Kernel Search Endpoint requests;
- Search Endpoint responses into the result shape expected by InstantSearch;
- pagination, facet/refinement, sorting, highlighting, and batching structures where Search Endpoints support them.

It must not independently define:

- query semantics;
- filter semantics;
- ranking semantics;
- facet/grouping semantics;
- Search Kernel defaults;
- field legality;
- index mutation or administration.

Those remain owned by Search Kernel's canonical protocol, IR, dispatcher, and backend path.

## Dependency

Implementation depends on a first-class **Search Endpoints** capability in `searchkerneld`: named, bounded, public-facing search endpoints that bind simple HTTP inputs into canonical Search Kernel requests without exposing the full trusted `Execute` surface.

The initial documentation-search application is expected to be the first end-to-end dogfood target.

## Status

Placeholder only. No implementation or supported package exists yet.

Before implementation lands, this integration must receive explicit ownership in the authoritative Search Kernel verification DAG, including selector coverage and full-suite reachability.
