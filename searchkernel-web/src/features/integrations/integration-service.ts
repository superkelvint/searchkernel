import compatibility from '../../../../integrations/instantsearch/src/compatibility.json'
import instantSearchPackage from '../../../../integrations/instantsearch/package.json'
import {
  createSearchKernelInstantSearchClient,
  SearchKernelInstantSearchError,
} from '../../../../integrations/instantsearch/src/client'
import {
  SearchKernelClientError,
  targetKey,
  type Index,
  type RequestOptions,
  type SearchKernelClient,
} from '../../api/searchkernel-client'

export const instantSearchCompatibility = compatibility
export const instantSearchAdapterVersion = instantSearchPackage.version

export const instantSearchInstallCommands = {
  vanilla: `npm install --save-exact instantsearch.js@${compatibility.packages['instantsearch.js']} @searchkernel/instantsearch@${instantSearchPackage.version}`,
  react: `npm install --save-exact react-instantsearch@${compatibility.packages['react-instantsearch']} react-instantsearch-core@${compatibility.packages['react-instantsearch-core']} react@${compatibility.packages.react} react-dom@${compatibility.packages['react-dom']} @searchkernel/instantsearch@${instantSearchPackage.version}`,
  vue: `npm install --save-exact vue-instantsearch@${compatibility.packages['vue-instantsearch']} vue@${compatibility.packages.vue} @searchkernel/instantsearch@${instantSearchPackage.version}`,
} as const

/** Validate through the existing generation-bound handle. Metadata alone may be cached.
 * This administrative read is identity validation, never the application preview.
 * Do not reopen on stale-generation errors or fall back to another listed target.
 */
export async function validateIntegrationIndex(client: SearchKernelClient, selected: Index, options?: RequestOptions): Promise<Index> {
  if (!selected.target) {
    throw new SearchKernelClientError('The client does not expose an authoritative index target. Integration identity cannot be verified in this mode.', { code: 'identity_unavailable' })
  }
  const targets = await client.listIndexes(options)
  if (!targets.some(target => target.id === selected.id)) {
    throw new SearchKernelClientError(`Logical index ${selected.id} is no longer available.`, { code: 'index_not_found' })
  }
  const current = await client.getIndex(selected.target, options)
  if (!current?.target || targetKey(current.target) !== targetKey(selected.target)) {
    throw new SearchKernelClientError('The selected index identity changed. Review the index selection before configuring an integration.', { code: 'stale_generation' })
  }
  // The daemon adapter retains the opened generation; this request checks it
  // against live registry state, including restart and delete/recreate changes.
  await client.browseDocuments(selected.target, { limit: 1 }, options)
  return current
}

export function normalizePublicSearchEndpoint(value: string): string {
  const candidate = value.trim()
  if (!candidate) throw new SearchKernelClientError('Enter the public Search Endpoint URL.', { code: 'public_endpoint_required' })
  let url: URL
  try { url = new URL(candidate) } catch {
    throw new SearchKernelClientError('Enter a valid http(s) public Search Endpoint URL ending in /search.', { code: 'invalid_public_endpoint' })
  }
  url.pathname = url.pathname.replace(/\/+$/, '')
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash || !url.pathname.endsWith('/search')) {
    throw new SearchKernelClientError('Use the bounded public Search Endpoint URL ending in /search, not the trusted administration listener.', { code: 'invalid_public_endpoint' })
  }
  return url.toString()
}

function publicAlias(value: string): string {
  const alias = value.trim()
  if (!alias) throw new SearchKernelClientError('Enter the configured public endpoint alias.', { code: 'public_alias_required' })
  return alias
}

export function instantSearchApplicationExamples(endpointValue: string, aliasValue: string) {
  const endpoint = normalizePublicSearchEndpoint(endpointValue)
  const alias = publicAlias(aliasValue)
  const endpointLiteral = JSON.stringify(endpoint)
  const aliasLiteral = JSON.stringify(alias)
  return {
    vanilla: `import instantsearch from 'instantsearch.js';
import { configure, searchBox, hits, pagination } from 'instantsearch.js/es/widgets';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

const search = instantsearch({
  indexName: ${aliasLiteral},
  insights: false,
  searchClient: createSearchKernelInstantSearchClient({ endpoint: ${endpointLiteral} }),
});
search.addWidgets([
  configure({ analytics: false, hitsPerPage: 20 }),
  searchBox({ container: '#query' }),
  hits({ container: '#hits' }),
  pagination({ container: '#pages' }),
]);
search.start();`,
    react: `import { InstantSearch, Configure, SearchBox, Hits, Pagination } from 'react-instantsearch';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

const searchClient = createSearchKernelInstantSearchClient({ endpoint: ${endpointLiteral} });

export function ProductSearch() {
  return <InstantSearch indexName=${aliasLiteral} searchClient={searchClient} insights={false}>
    <Configure analytics={false} hitsPerPage={6}/>
    <SearchBox/><Hits/><Pagination/>
  </InstantSearch>;
}`,
    vue: `<script setup>
import { AisInstantSearch, AisConfigure, AisSearchBox, AisHits, AisPagination } from 'vue-instantsearch/vue3/es';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

const searchClient = createSearchKernelInstantSearchClient({ endpoint: ${endpointLiteral} });
const indexName = ${aliasLiteral};
</script>

<template>
  <ais-instant-search :index-name="indexName" :search-client="searchClient" :insights="false">
    <ais-configure :analytics="false" :hits-per-page="6"/>
    <ais-search-box/><ais-hits/><ais-pagination/>
  </ais-instant-search>
</template>`,
  } as const
}

/** Execute the real landed InstantSearch adapter against an explicitly supplied
 * bounded public Search Endpoint. This proves the public path, not that the
 * supplied alias is canonically bound to the currently selected admin target.
 */
export async function runInstantSearchPreview(
  endpointValue: string,
  aliasValue: string,
  query: string,
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<Record<string, unknown>> {
  const endpoint = normalizePublicSearchEndpoint(endpointValue)
  const alias = publicAlias(aliasValue)
  try {
    const client = createSearchKernelInstantSearchClient({ endpoint, timeoutMs: 5_000, fetch: fetcher })
    const response = await client.search([{ indexName: alias, params: { query, hitsPerPage: 5, analytics: false } }])
    const result = response.results[0]
    if (!result) throw new SearchKernelClientError('The public Search Endpoint returned no preview result.', { code: 'invalid_response' })
    return result as unknown as Record<string, unknown>
  } catch (error) {
    if (error instanceof SearchKernelClientError) throw error
    if (error instanceof SearchKernelInstantSearchError) {
      throw new SearchKernelClientError(error.message, { code: error.code, retryable: ['unavailable', 'timeout', 'overloaded'].includes(error.code), origin: 'client', details: error.requestIndex === undefined ? undefined : { requestIndex: error.requestIndex } })
    }
    throw error
  }
}
