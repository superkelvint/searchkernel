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

## Hard compatibility contract

InstantSearch.js compatibility is the product reason for the Search Endpoint request API. It is therefore a merge requirement, not a future nice-to-have.

The first server-side compatibility profile is deliberately small and explicit:

- `query` maps to canonical Search Kernel search text;
- `hitsPerPage` maps to the canonical result limit;
- zero-based `page` maps to checked canonical offset arithmetic;
- an empty or omitted `query` maps to canonical match-all browse;
- the general Search Endpoint aliases `q`, `limit`, and `offset` must not create different semantics;
- ambiguous alias combinations, page/offset conflicts, overflow, malformed values, and policy-limit violations fail closed;
- accepted requests must traverse the canonical Dispatcher -> `*_to_ir()` path.

This profile is owned by the dedicated `instantsearch-compat` verification task. A generic Search Endpoint unit suite is not sufficient evidence.

Full compatibility is larger than this first request profile. When the adapter lands, acceptance must additionally exercise the actual pinned InstantSearch.js search-client contract rather than only handwritten lookalike fixtures, including `search(requests)` batching, result ordering, pagination metadata, facets/refinements, sort choices, highlighting/snippets, empty-query behavior, and error handling for the supported widget set.

## Dependency

Implementation depends on a first-class **Search Endpoints** capability in `searchkerneld`: named, bounded, public-facing search endpoints that bind simple HTTP inputs into canonical Search Kernel requests without exposing the full trusted `Execute` surface.

The initial documentation-search application is expected to be the first end-to-end dogfood target.

## Status

The adapter package is still a placeholder and is not yet a supported integration. The server-side core request compatibility gate is now part of the Search Endpoint acceptance surface.

The authoritative verification DAG owns `integrations/instantsearch/**` and the dedicated compatibility verifier. Full adapter support must not be claimed until a real InstantSearch.js-driven acceptance harness and the supported response/widget surface are executable.
