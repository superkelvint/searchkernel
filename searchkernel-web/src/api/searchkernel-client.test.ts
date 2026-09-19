import { describe, expect, it } from 'vitest'
import { createFixtureSearchKernelClient } from './fixture-searchkernel-client'
import { initialSearchRequest, normalizeClientError, SearchKernelClientError } from './searchkernel-client'

describe('SearchKernelClient boundary', () => {
  const client = createFixtureSearchKernelClient()

  it('returns distinct indexes through the client contract', async () => {
    const indexes = await client.listIndexes()
    expect(indexes.map(index => index.id)).toEqual(['products', 'articles', 'users'])
    expect((await client.getDocument('products', 'sku-42'))?.id).toBe('sku-42')
  })

  it('echoes the request that drove a search response', async () => {
    const request = { ...initialSearchRequest, query: 'headphones', ranking: 'semantic', limit: 10 }
    const response = await client.search('products', request)
    expect(response.request).toEqual(request)
    expect(response.hits.length).toBeGreaterThan(0)
    expect(response.resolved).toContain('semantic')
  })

  it('exposes capabilities and only explicit analysis responses', async () => {
    const capabilities = await client.capabilities()
    expect(capabilities.schema.write).toBe(false)
    expect(capabilities.search.facets).toBe(false)
    expect(await client.analyze('products', 'Running with the foxes')).toHaveLength(4)
    expect(await client.analyze('products', 'unlisted input')).toEqual([])
  })

  it('normalizes operational failures into structured errors', () => {
    const error = normalizeClientError(new SearchKernelClientError('offline', { code: 'transport', status: 503, retryable: true }))
    expect(error).toMatchObject({ code: 'transport', status: 503, retryable: true, message: 'offline' })
  })
})
