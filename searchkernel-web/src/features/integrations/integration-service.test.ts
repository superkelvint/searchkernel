import { describe, expect, it, vi } from 'vitest'
import { IndexTargetT, SearchKernelClientError, type Index, type SearchKernelClient } from '../../api/searchkernel-client'
import { isIndexPage, navigationFor, pagePath, tabFor } from '../../app/navigation'
import {
  instantSearchAdapterVersion,
  instantSearchApplicationExamples,
  instantSearchCompatibility,
  instantSearchInstallCommands,
  runInstantSearchPreview,
  validateIntegrationIndex,
} from './integration-service'

const selected: Index = { id: 'catalog', generation: 'incarnation-1', target: new IndexTargetT('catalog', 'incarnation-1'), fields: [], profiles: [], summaries: [], searchDefaults: {} }
function boundary(current: Index | undefined = selected) {
  return {
    listIndexes: vi.fn().mockResolvedValue([{ id: 'catalog' }, { id: 'other' }]),
    getIndex: vi.fn().mockResolvedValue(current),
    browseDocuments: vi.fn().mockResolvedValue({ documents: [] }),
  } as unknown as SearchKernelClient
}

describe('W6 selected index identity', () => {
  it('validates only the selected generation through the existing client with cancellation', async () => {
    const client = boundary(), options = { signal: new AbortController().signal }
    expect(await validateIntegrationIndex(client, selected, options)).toBe(selected)
    expect(client.getIndex).toHaveBeenCalledExactlyOnceWith(selected.target, options)
    expect(client.browseDocuments).toHaveBeenCalledExactlyOnceWith(selected.target, { limit: 1 }, options)
  })
  it('rejects deletion even when old metadata is still cached; never selects the remaining target', async () => {
    const client = boundary()
    vi.mocked(client.listIndexes).mockResolvedValue([{ id: 'other' }])
    await expect(validateIntegrationIndex(client, selected)).rejects.toMatchObject({ code: 'index_not_found', retryable: false })
    expect(client.getIndex).not.toHaveBeenCalled()
    expect(client.browseDocuments).not.toHaveBeenCalled()
  })
  it.each([
    { ...selected, generation: 'incarnation-2', target: new IndexTargetT('catalog', 'incarnation-2') },
    { ...selected, id: 'other', target: new IndexTargetT('other', 'incarnation-1') },
  ])('rejects replacement identity instead of accepting a new target', async current => {
    const client = boundary(current)
    await expect(validateIntegrationIndex(client, selected)).rejects.toMatchObject({ code: 'stale_generation' })
    expect(client.browseDocuments).not.toHaveBeenCalled()
  })
  it('does not treat a generation-free offline fixture as verified', async () => {
    const fixture = { ...selected, generation: undefined, target: undefined }, client = boundary(fixture)
    await expect(validateIntegrationIndex(client, fixture)).rejects.toMatchObject({ code: 'identity_unavailable' })
    expect(client.browseDocuments).not.toHaveBeenCalled()
  })
  it('surfaces live stale-generation errors even when getIndex returns cached metadata', async () => {
    const client = boundary()
    const error = new SearchKernelClientError('stale_index_handle', { code: 'failed_precondition', retryable: false })
    vi.mocked(client.browseDocuments).mockRejectedValue(error)
    await expect(validateIntegrationIndex(client, selected)).rejects.toBe(error)
  })
})

describe('W6 landed InstantSearch authority', () => {
  it('derives exact install commands and framework examples from the canonical package declaration', () => {
    expect(instantSearchCompatibility.releaseStatus).toBe('qualified')
    expect(instantSearchInstallCommands.vanilla).toContain(`instantsearch.js@${instantSearchCompatibility.packages['instantsearch.js']}`)
    expect(instantSearchInstallCommands.vanilla).toContain(`@searchkernel/instantsearch@${instantSearchAdapterVersion}`)
    expect(instantSearchInstallCommands.react).toContain(`react-instantsearch@${instantSearchCompatibility.packages['react-instantsearch']}`)
    expect(instantSearchInstallCommands.vue).toContain(`vue-instantsearch@${instantSearchCompatibility.packages['vue-instantsearch']}`)
    const examples = instantSearchApplicationExamples('https://search.example.com/search', 'products')
    expect(examples.vanilla).toContain("createSearchKernelInstantSearchClient")
    expect(examples.vanilla).toContain('indexName: "products"')
    expect(examples.react).toContain('endpoint: "https://search.example.com/search"')
    expect(examples.vue).toContain('vue-instantsearch/vue3/es')
  })

  it('runs preview through the real adapter translation and projection boundary', async () => {
    const fetcher = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ results: [{
        endpoint: 'products', query: 'orchid', totalHits: 1, exact: true, processingTimeMs: 2,
        facets: {}, exhaustiveFacetsCount: true,
        hits: [{ id: 'one', fields: { title: 'orchid' }, highlights: {} }],
      }] }),
    }) as unknown as typeof fetch
    const result = await runInstantSearchPreview('https://search.example.com/search', 'products', 'orchid', fetcher)
    expect(result).toMatchObject({ index: 'products', query: 'orchid', nbHits: 1, hits: [{ objectID: 'one', title: 'orchid' }] })
    expect(fetcher).toHaveBeenCalledTimes(1)
    const [, init] = vi.mocked(fetcher).mock.calls[0]
    const body = JSON.parse(String(init?.body))
    expect(body.searches).toEqual([{ endpoint: 'products', query: 'orchid', limit: 5, offset: 0, facets: [], facetLimit: 100, filters: [], snippets: [] }])
  })

  it('rejects trusted/admin-shaped URLs before the adapter can send a request', async () => {
    const fetcher = vi.fn() as unknown as typeof fetch
    await expect(runInstantSearchPreview('http://127.0.0.1:8081/v1/execute', 'products', '', fetcher)).rejects.toMatchObject({ code: 'invalid_public_endpoint' })
    expect(fetcher).not.toHaveBeenCalled()
  })

  it('preserves canonical public endpoint error codes from the adapter', async () => {
    const fetcher = vi.fn().mockResolvedValue({ ok: false, status: 409, json: async () => ({ error: { code: 'stale_target' } }) }) as unknown as typeof fetch
    await expect(runInstantSearchPreview('https://search.example.com/search', 'products', '', fetcher)).rejects.toMatchObject({ code: 'stale_target', retryable: false })
  })
})

describe('W6 navigation', () => {
  it('requires selected index context and exposes Integrations as an index tab', () => {
    expect(isIndexPage('integrations')).toBe(true)
    expect(tabFor('integrations')).toBe('Integrations')
    expect(pagePath('integrations', 'catalog')).toBe('/indexes/catalog/integrations/instantsearch')
    expect(pagePath('integrations', 'other')).toBe('/indexes/other/integrations/instantsearch')
    expect(() => pagePath('integrations')).toThrow('index id is required')
    expect(navigationFor().global.map(item => item.page)).not.toContain('integrations')
    expect(navigationFor().sections.Integrations).toEqual([{ label: 'InstantSearch.js', page: 'integrations', visible: true, enabled: false, reason: 'Capabilities are loading.' }])
  })
})
