# Search Kernel InstantSearch integration

This independent adapter connects external applications to Search Kernel's bounded public search listener. Search execution uses canonical FlatBuffers `SearchRequest`, the multi-index registry, dispatcher, shared IR resolver, and Vespa. It does not require `searchkernel-web`.

The compatibility promise is **compatibility with the declared pinned InstantSearch.js/search-client contract and tested supported-widget matrix**. It is not general Algolia API compatibility. The authoritative declaration is [src/compatibility.json](src/compatibility.json). `releaseStatus: unverified` means acceptance has not qualified a release; rendering a demonstration is insufficient.

## Application integration

```sh
npm install --save-exact instantsearch.js@4.119.0 @searchkernel/instantsearch@0.1.0
```

The package is built here; npm publication is a separate release operation. Before publication, install the archive produced by `npm pack` in this directory instead of the registry package.

```ts
import instantsearch from 'instantsearch.js';
import { configure, searchBox, hits, pagination } from 'instantsearch.js/es/widgets';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

const search = instantsearch({
  indexName: 'products',
  insights: false,
  searchClient: createSearchKernelInstantSearchClient({
    endpoint: 'https://search.example.com/search',
    timeoutMs: 5000,
  }),
});
search.addWidgets([
  configure({analytics: false, hitsPerPage: 20}),
  searchBox({container: '#query'}),
  hits({container: '#hits'}),
  pagination({container: '#pages'}),
]);
search.start();
```

`search(requests)` sends one bounded POST and returns results in input order. Each request has its own index, query, refinements, pagination, and projection. Maximum batch size is 16. Validation failure rejects before sending; server failure rejects the whole batch with the earliest failing `requestIndex`. There are no response caches or mutable current-index slots. Overlapping calls have separate buffers and deadlines. The pinned consumer owns UI request ordering; aborting HTTP does not claim to cancel native work.

## Public listener

Create/open indexes through the trusted canonical lifecycle API first. Obtain generation tokens from `OpenIndexResult`; public serving never creates, opens, deletes, or mutates indexes. Each public name maps to an explicit canonical target and fixed generation. A deleted/recreated target requires updated configuration.

```json
{
  "behindRateLimitedProxy": true,
  "allowedOrigins": ["https://app.example.com"],
  "endpoints": {
    "products": {
      "target": "products",
      "generation": "<generation returned by OpenIndex>",
      "fields": ["title", "description", "brand", "price"],
      "filterFields": {"brand": "string", "price": "int64"},
      "facets": ["brand"],
      "highlights": ["title", "description"],
      "sort": []
    }
  }
}
```

```sh
searchkerneld --data-root /srv/searchkernel --listen 127.0.0.1:9000 \
  --public-search-config public-search.json --public-search-listen 127.0.0.1:9001
```

Both public flags are required; serving is disabled by default. The separate public listener accepts only `POST /search` and CORS preflight. Trusted Execute, schema, mutation, administration, and arbitrary native properties are absent. Deploy behind TLS termination and rate limiting; `behindRateLimitedProxy` acknowledges this required deployment boundary. CORS is not authentication; origins are explicit and wildcard origins are rejected.

The body is `{searches: [...]}`. Each search contains `endpoint`, `query`, `limit`, `offset`, `facets`, `facetLimit`, `filters`, optional `fields` and `highlights`, and `snippets`. Applications normally use the adapter to build it.

| Public representation | Canonical mapping |
| --- | --- |
| endpoint | configured IndexTarget name and fixed generation |
| query | SearchKernelTextQuery; empty/whitespace-only text becomes MatchAllQuery |
| limit / offset | explicit backend limit / offset; no downloaded hit slicing |
| filters | typed FilterComparison / FilterOr / FilterAnd in FilterRequest.structured |
| facets / facetLimit | structured count grouping with backend ordering and limit |
| configured sort | FieldSortTarget and direction |
| fields | PresentationRequest.fields allowlisted projection |
| highlights / snippets | configured HighlightRequest; Vespa selects excerpts and match spans |
| timeout | canonical timeout_ms plus bounded HTTP wait |

Facet counts cover the complete filtered query, not the returned page. The real helper creates independent searches for disjunctive counts. Wildcards expand only to configured public fields. Numeric comparisons retain inclusive/exclusive operators and enter canonical typed filters. `sortBy` values are explicit endpoint aliases with configured canonical sorts; they are not arbitrary replica names. Each alias carries its target, generation, allowlists, and fixed sort.

## Result and presentation semantics

Canonical hit IDs become `objectID`; allowlisted fields become hit attributes. Canonical totals/relation become `nbHits`/`exhaustiveNbHits`. `nbPages = ceil(nbHits / hitsPerPage)`, or zero for a facet-only zero-size request. Current page remains the requested zero-based page, including beyond available results. `query` and `index` retain request identity. `processingTimeMS` measures dispatch/projection wall time. Canonical count groups become `facets`; `exhaustiveFacetsCount` is conservatively false when the grouping limit may have been reached or totals are not exact.

Highlight and Snippet are distinct properties over backend-selected Juniper excerpts and canonical byte spans, including unhighlighted excerpts. This profile does not promise full-field Algolia highlighting or Algolia word-count truncation. `:wordCount` snippet directives, nested/multi-valued presentation, and custom markup are unsupported. Missing fields have no result entry. `matchLevel` is `partial` when spans prove a match and `none` otherwise; `matchedWords` records those surface spans without inventing query analysis.

Keep Hits' default `escapeHTML: true`. The adapter inserts only the pinned consumer's placeholders at backend spans; the consumer escapes text and turns placeholders into `<mark>`. Chromium acceptance checks HTML-sensitive values. There is no substring highlighting or client-side snippet truncation.

## Bounds and errors

The server bounds headers, URI, body (64 KiB), batch (16), query (4096 UTF-8 bytes), filter tree (128 nodes, depth 4), field lists, facets (100), hit limit (100), offset (10000), canonical/result payload (1 MiB), connections (64), admission (32), and execution (8). Header/body reads and execution waits have deadlines. Native tasks retain execution permits after HTTP timeout/disconnect. Batches execute sequentially in one native slot and stop starting searches after the deadline.

`SearchKernelInstantSearchError` exposes `code` and optional `requestIndex`. Stable codes: `invalid_request`, `unsupported`, `unknown_index`, `stale_target`, `unavailable`, `timeout`, `overloaded`, `backend_failure`, and adapter-only `invalid_response`. Messages are fixed. Canonical details, filesystem paths, panic text, and stack traces are not returned. Errors never become successful empty results.

Searchable refinement lists, facet-value search, hierarchical facets, arbitrary Algolia textual `filters`, ranking controls, geo search, recommendations, analytics, and replicas are explicitly excluded. React/Vue bindings are not advertised by this profile.

## Verification

```sh
./dev test instantsearch-compat
```

The canonical containerized lane runs pin/matrix/documentation guards, upstream type checks, adapter units, canonical binding checks, initial request-profile regressions, actual pinned widgets against the public listener/dispatcher/Vespa path, and Chromium UI acceptance. The same lane runs in PR CI and Universe.

Changing an ecosystem pin requires updating the declaration and lock and passing acceptance. CI fails on pin, installed-version, upstream widget-inventory, matrix-coverage, or generated-document drift. The production adapter has no dashboard or framework runtime dependency.

<!-- compatibility-matrix:start -->
| Pinned upstream package | Version |
| --- | --- |
| instantsearch.js | 4.119.0 |
| algoliasearch-helper | 3.30.0 |
| algoliasearch | 5.59.0 |
| @algolia/client-search | 5.59.0 |

| Upstream widget | Status | Coverage / reason |
| --- | --- | --- |
| EXPERIMENTAL_answers | unsupported | Outside the declared backend and acceptance profile |
| EXPERIMENTAL_autocomplete | unsupported | Outside the declared backend and acceptance profile |
| EXPERIMENTAL_configureRelatedItems | unsupported | Outside the declared backend and acceptance profile |
| EXPERIMENTAL_dynamicWidgets | unsupported | Outside the declared backend and acceptance profile |
| analytics | unsupported | Outside the declared backend and acceptance profile |
| autocomplete | unsupported | Outside the declared backend and acceptance profile |
| breadcrumb | unsupported | Outside the declared backend and acceptance profile |
| chat | unsupported | Outside the declared backend and acceptance profile |
| chatTrigger | unsupported | Outside the declared backend and acceptance profile |
| clearRefinements | supported with documented constraints | real-engine:clearRefinements |
| compareBar | unsupported | Outside the declared backend and acceptance profile |
| configure | supported with documented constraints | real-engine:configure |
| currentRefinements | supported with documented constraints | real-engine:currentRefinements |
| dynamicWidgets | unsupported | Outside the declared backend and acceptance profile |
| feeds | unsupported | Outside the declared backend and acceptance profile |
| filterSuggestions | unsupported | Outside the declared backend and acceptance profile |
| frequentlyBoughtTogether | unsupported | Outside the declared backend and acceptance profile |
| geoSearch | unsupported | Outside the declared backend and acceptance profile |
| hierarchicalMenu | unsupported | Outside the declared backend and acceptance profile |
| hits | supported with documented constraints | real-engine:hits |
| hitsPerPage | supported with documented constraints | real-engine:hitsPerPage |
| index | supported with documented constraints | real-engine:index |
| infiniteHits | unsupported | Outside the declared backend and acceptance profile |
| lookingSimilar | unsupported | Outside the declared backend and acceptance profile |
| menu | supported with documented constraints | real-engine:menu |
| menuSelect | unsupported | Outside the declared backend and acceptance profile |
| numericMenu | supported with documented constraints | real-engine:numericMenu |
| pagination | supported with documented constraints | real-engine:pagination |
| panel | unsupported | Outside the declared backend and acceptance profile |
| places | unsupported | Outside the declared backend and acceptance profile |
| poweredBy | unsupported | Outside the declared backend and acceptance profile |
| promptSuggestions | unsupported | Outside the declared backend and acceptance profile |
| queryRuleContext | unsupported | Outside the declared backend and acceptance profile |
| queryRuleCustomData | unsupported | Outside the declared backend and acceptance profile |
| rangeInput | unsupported | Outside the declared backend and acceptance profile |
| rangeSlider | unsupported | Outside the declared backend and acceptance profile |
| ratingMenu | unsupported | Outside the declared backend and acceptance profile |
| refinementList | supported with documented constraints | real-engine:refinementList |
| relatedProducts | unsupported | Outside the declared backend and acceptance profile |
| relevantSort | unsupported | Outside the declared backend and acceptance profile |
| resultCard | unsupported | Outside the declared backend and acceptance profile |
| searchBox | supported with documented constraints | real-engine:searchBox |
| sortBy | supported with documented constraints | real-engine:sortBy |
| stats | supported with documented constraints | real-engine:stats |
| toggleRefinement | unsupported | Outside the declared backend and acceptance profile |
| trendingFacets | unsupported | Outside the declared backend and acceptance profile |
| trendingItems | unsupported | Outside the declared backend and acceptance profile |
| voiceSearch | unsupported | Outside the declared backend and acceptance profile |
<!-- compatibility-matrix:end -->
