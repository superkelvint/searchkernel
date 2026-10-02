import { describe, expect, it } from 'vitest'
import { createFixtureSearchKernelClient } from '../../api/fixture-searchkernel-client'
import { readSearchParams } from './search-params'

describe('search lab URL state', () => {
  it('accepts supported request values', async () => {
    const index = (await createFixtureSearchKernelClient().getIndex('products'))!
    expect(readSearchParams(new URLSearchParams('q=sony&ranking=semantic&limit=10&summary=compact'), index)).toEqual({ query: 'sony', ranking: 'semantic', limit: 10, summary: 'compact' })
  })
  it('preserves explicit controls for canonical validation', async () => {
    const index = (await createFixtureSearchKernelClient().getIndex('products'))!
    expect(readSearchParams(new URLSearchParams('ranking=missing&limit=3&summary=missing'), index)).toEqual({ query: '', ranking: 'missing', limit: 3, summary: 'missing' })
  })
})
