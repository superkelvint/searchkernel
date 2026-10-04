# Search Kernel InstantSearch integration

This independent adapter connects external applications to Search Kernel's bounded public search listener. Search execution uses canonical FlatBuffers `SearchRequest`, the multi-index registry, dispatcher, shared IR resolver, and Vespa. It does not require `searchkernel-web`.

The compatibility promise is **compatibility with the declared pinned InstantSearch.js/search-client contract and tested supported-widget matrix**. It is not general Algolia API compatibility. The authoritative declaration is [src/compatibility.json](src/compatibility.json). `releaseStatus: qualified` records canonical real-engine JSDOM qualification for these exact package versions and declared client-side surfaces. It is not a package-publication or unrestricted Algolia compatibility claim.

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


## React InstantSearch

```sh
npm install --save-exact react-instantsearch@7.51.0 react-instantsearch-core@7.51.0 react@19.3.0 react-dom@19.3.0 @searchkernel/instantsearch@0.1.0
```

```tsx
import { InstantSearch, Configure, SearchBox, Hits, Highlight, Snippet, Pagination } from 'react-instantsearch';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

// Keep this reference outside renders (or memoize it).
const searchClient = createSearchKernelInstantSearchClient({endpoint: 'https://search.example.com/search'});
function Product({hit}) {
  return <article><Highlight attribute="title" hit={hit}/><Snippet attribute="description" hit={hit}/></article>;
}
export function ProductSearch() {
  return <InstantSearch indexName="products" searchClient={searchClient} insights={false}>
    <Configure analytics={false} hitsPerPage={6} attributesToHighlight={['title']} attributesToSnippet={['description']}/>
    <SearchBox/><Hits hitComponent={Product}/><Pagination/>
  </InstantSearch>;
}
```

React 7.51.0 provides `useNumericMenu`, not a ready-made `NumericMenu` export. Numeric-menu qualification uses that real hook's items/refine interface with ordinary rendering. React search-client prop replacement is qualified. Server rendering, hydration and routing are outside this client-side profile.

## Vue InstantSearch (Vue 3)

```sh
npm install --save-exact vue-instantsearch@4.30.0 vue@3.5.43 @searchkernel/instantsearch@0.1.0
```

```vue
<script setup>
import { AisInstantSearch, AisConfigure, AisSearchBox, AisHits, AisHighlight, AisSnippet, AisPagination } from 'vue-instantsearch/vue3/es';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';
const searchClient = createSearchKernelInstantSearchClient({endpoint: 'https://search.example.com/search'});
</script>
<template>
  <ais-instant-search index-name="products" :search-client="searchClient" :insights="false">
    <ais-configure :analytics="false" :hits-per-page="6" :attributes-to-highlight="['title']" :attributes-to-snippet="['description']"/>
    <ais-search-box/>
    <ais-hits><template #item="{item}">
      <ais-highlight attribute="title" :hit="item"/><ais-snippet attribute="description" :hit="item"/>
    </template></ais-hits>
    <ais-pagination/>
  </ais-instant-search>
</template>
```

Vue qualification is for Vue 3 and the `vue-instantsearch/vue3/es` entry point, not Vue 2 or SSR. **Keep the search-client reference stable for the root's lifetime.** In pinned Vue 4.30.0, its prop watcher calls `instantSearchInstance.helper.setClient(...).search()` while execution remains on the original main helper/client. Real-engine acceptance records that the replacement client receives no requests. Dynamic client replacement is excluded; remount the root when changing client configuration. This is an upstream lifecycle constraint, not a separate Search Kernel adapter.

React InstantSearch and Vue InstantSearch are supported for the widgets and request semantics declared in this profile. Components requiring unsupported underlying features remain unsupported: hierarchical navigation, infinite hits, range widgets, toggle refinement, facet-value search/searchable lists, geo search, dynamic widgets, analytics/insights and recommendations. Framework bindings do not add backend semantics. Keep default hit escaping enabled. Advanced Configure numericFilters must not be mixed with managed numeric refinements on the same field; the upstream helper rejects this combination.

The adapter package has no framework runtime dependency. As with vanilla installation, use the locally packed archive until the package is published.

`search(requests)` sends one bounded POST and returns results in input order. Each request has its own index, query, refinements, pagination, and projection. Maximum batch size is 16. Validation failure rejects before sending; the endpoint validates the whole batch before dispatch, then rejects on the first execution failure in input order; `requestIndex` identifies the failure. There are no response caches or mutable current-index slots. Overlapping calls have separate buffers and deadlines. The pinned consumer owns UI request ordering; aborting HTTP does not claim to cancel native work.

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

Keep Hits' default `escapeHTML: true`. The adapter inserts only the pinned consumer's placeholders at backend spans; the consumer escapes text and turns placeholders into `<mark>`. The real-widget JSDOM acceptance checks HTML-sensitive values. There is no substring highlighting or client-side snippet truncation.

## Bounds and errors

The server bounds headers, URI, body (64 KiB), batch (16), query (4096 UTF-8 bytes), filter tree (128 nodes, depth 4), field lists, facets (100), hit limit (100), offset (10000), canonical/result payload (1 MiB), connections (64), admission (32), and execution (8). Header/body reads and execution waits have deadlines. Native tasks retain execution permits after HTTP timeout/disconnect. Batches execute sequentially in one native slot and stop starting searches after the deadline.

`SearchKernelInstantSearchError` exposes `code` and optional `requestIndex`. Stable codes: `invalid_request`, `unsupported`, `unknown_index`, `stale_target`, `unavailable`, `timeout`, `overloaded`, `backend_failure`, and adapter-only `invalid_response`. Messages are fixed. Canonical details, filesystem paths, panic text, and stack traces are not returned. Errors never become successful empty results.

Searchable refinement lists, facet-value search, hierarchical facets, arbitrary Algolia textual `filters`, ranking controls, geo search, recommendations, analytics, and replicas are explicitly excluded. React/Vue qualification inherits these exclusions and the framework constraints above. Widget status describes tested support; the search-client boundary cannot identify a rendering-only widget. An unlisted/unsupported widget has no compatibility guarantee even when it emits allowed parameters. Unsupported methods and parameters fail explicitly.

## Verification

```sh
./dev test instantsearch-compat
```

The canonical containerized lane runs pin/matrix/documentation guards, upstream type checks, adapter units, canonical binding checks, initial request-profile regressions, and the actual pinned InstantSearch.js widgets in Vitest/JSDOM against the public listener/dispatcher/Vespa path. The same lane runs in PR CI and Universe; no browser binary is required.

Changing an ecosystem pin requires updating the declaration and lock and passing acceptance. CI fails on pin, installed-version, nested ecosystem-version, upstream source/widget-inventory, framework qualification registry, component-coverage, or generated-document drift. The executable registry loads both actual framework mounts; runtime acceptance checks every supported widget/helper is mounted and behavior asserted. The production adapter has no dashboard or framework runtime dependency.

<!-- compatibility-matrix:start -->
| Pinned upstream package | Version |
| --- | --- |
| instantsearch.js | 4.119.0 |
| algoliasearch-helper | 3.30.0 |
| algoliasearch | 5.59.0 |
| @algolia/client-search | 5.59.0 |
| react-instantsearch | 7.51.0 |
| react-instantsearch-core | 7.51.0 |
| vue-instantsearch | 4.30.0 |
| react | 19.3.0 |
| react-dom | 19.3.0 |
| vue | 3.5.43 |

| Framework | Status | Qualification / constraints |
| --- | --- | --- |
| react-instantsearch | supported with documented constraints | react-profile; Client-side only; inherits the declared scalar fields, bounded request semantics and supported widget/helper profile. SSR, hydration, routing and unsupported underlying capabilities are not qualified. Configure numericFilters must not share a field with a managed NumericMenu refinement. Numeric menu uses the actual useNumericMenu hook; this version has no NumericMenu UI export. Search-client prop replacement is qualified. |
| vue-instantsearch | supported with documented constraints | vue3-profile; Client-side only; inherits the declared scalar fields, bounded request semantics and supported widget/helper profile. SSR, hydration, routing and unsupported underlying capabilities are not qualified. Configure numericFilters must not share a field with a managed NumericMenu refinement. Vue 3 only, using vue-instantsearch/vue3/es. Keep a stable search-client reference: this pinned Vue watcher updates helper.client rather than the executing mainHelper; dynamic prop replacement is not supported by this qualification. |

| Underlying capability | React surface | Vue 3 surface |
| --- | --- | --- |
| Highlight | Highlight | AisHighlight |
| Snippet | Snippet | AisSnippet |
| clearRefinements | ClearRefinements | AisClearRefinements |
| configure | Configure | AisConfigure |
| currentRefinements | CurrentRefinements | AisCurrentRefinements |
| hits | Hits | AisHits |
| hitsPerPage | HitsPerPage | AisHitsPerPage |
| index | Index | AisIndex |
| menu | Menu | AisMenu |
| numericMenu | useNumericMenu | AisNumericMenu |
| pagination | Pagination | AisPagination |
| refinementList | RefinementList | AisRefinementList |
| searchBox | SearchBox | AisSearchBox |
| sortBy | SortBy | AisSortBy |
| stats | Stats | AisStats |

| Upstream widget | Status | Coverage / constraints / reason |
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
| clearRefinements | supported with documented constraints | real-engine:clearRefinements; Declared scalar facet and numeric refinements only |
| compareBar | unsupported | Outside the declared backend and acceptance profile |
| configure | supported with documented constraints | real-engine:configure; Declared request parameter allowlist only; analytics:false |
| currentRefinements | supported with documented constraints | real-engine:currentRefinements; Declared scalar facet and numeric refinements only |
| dynamicWidgets | unsupported | Outside the declared backend and acceptance profile |
| feeds | unsupported | Outside the declared backend and acceptance profile |
| filterSuggestions | unsupported | Outside the declared backend and acceptance profile |
| frequentlyBoughtTogether | unsupported | Outside the declared backend and acceptance profile |
| geoSearch | unsupported | Outside the declared backend and acceptance profile |
| hierarchicalMenu | unsupported | Outside the declared backend and acceptance profile |
| hits | supported with documented constraints | real-engine:hits; escapeHTML:true; configured fields; Highlight/Snippet use scalar backend excerpts |
| hitsPerPage | supported with documented constraints | real-engine:hitsPerPage; Integer page sizes 1..100; checked offset <=10000 |
| index | supported with documented constraints | real-engine:index; Configured public aliases only; combined batches <=16 |
| infiniteHits | unsupported | Outside the declared backend and acceptance profile |
| lookingSimilar | unsupported | Outside the declared backend and acceptance profile |
| menu | supported with documented constraints | real-engine:menu; One selected scalar value of a configured facet |
| menuSelect | unsupported | Outside the declared backend and acceptance profile |
| numericMenu | supported with documented constraints | real-engine:numericMenu; Configured numeric fields and typed inclusive/exclusive comparisons; no unrefined numeric statistics |
| pagination | supported with documented constraints | real-engine:pagination; Canonical backend limit/offset; checked offset <=10000 |
| panel | unsupported | Outside the declared backend and acceptance profile |
| places | unsupported | Outside the declared backend and acceptance profile |
| poweredBy | unsupported | Outside the declared backend and acceptance profile |
| promptSuggestions | unsupported | Outside the declared backend and acceptance profile |
| queryRuleContext | unsupported | Outside the declared backend and acceptance profile |
| queryRuleCustomData | unsupported | Outside the declared backend and acceptance profile |
| rangeInput | unsupported | Outside the declared backend and acceptance profile |
| rangeSlider | unsupported | Outside the declared backend and acceptance profile |
| ratingMenu | unsupported | Outside the declared backend and acceptance profile |
| refinementList | supported with documented constraints | real-engine:refinementList; Configured scalar facets; OR/AND; searchable:false; no hierarchical refinements |
| relatedProducts | unsupported | Outside the declared backend and acceptance profile |
| relevantSort | unsupported | Outside the declared backend and acceptance profile |
| resultCard | unsupported | Outside the declared backend and acceptance profile |
| searchBox | supported with documented constraints | real-engine:searchBox; Canonical text queries <=4096 UTF-8 bytes; insights:false; empty text browses |
| sortBy | supported with documented constraints | real-engine:sortBy; Configured aliases with fixed canonical sorts; no arbitrary replica semantics |
| stats | supported with documented constraints | real-engine:stats; Canonical total/relation and dispatch/projection wall time |
| toggleRefinement | unsupported | Outside the declared backend and acceptance profile |
| trendingFacets | unsupported | Outside the declared backend and acceptance profile |
| trendingItems | unsupported | Outside the declared backend and acceptance profile |
| voiceSearch | unsupported | Outside the declared backend and acceptance profile |
<!-- compatibility-matrix:end -->
