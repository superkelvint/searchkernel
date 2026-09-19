import { describe, expect, it } from 'vitest'
import { createFixtureSearchKernelClient } from '../api/fixture-searchkernel-client'
import { navigationFor, pagePath, tabFor } from './navigation'

describe('dashboard navigation', () => {
  it('builds canonical global and index paths', () => {
    expect(pagePath('global-overview')).toBe('/overview')
    expect(pagePath('index-overview', 'articles')).toBe('/indexes/articles/overview')
    expect(pagePath('documents', 'products')).toBe('/indexes/products/data/documents')
    expect(pagePath('diagnostics', 'users')).toBe('/indexes/users/operations/diagnostics')
  })

  it('maps workspace pages to capability-driven tabs and sections', async () => {
    const capabilities = await createFixtureSearchKernelClient().capabilities()
    const navigation = navigationFor(capabilities)
    expect(tabFor('global-overview')).toBeNull()
    expect(tabFor('search')).toBe('Search')
    expect(tabFor('schema')).toBe('Configuration')
    expect(navigation.sections.Search.map(section => section.page)).toEqual(['search', 'analyze', 'inspect'])
    expect(navigation.sections.Configuration.map(section => section.page)).toEqual(['schema', 'ranking', 'summaries'])
    expect(navigation.sections.Search.find(section => section.page === 'search')?.enabled).toBe(true)
    expect(navigationFor({ ...capabilities, search: { ...capabilities.search, basic: false } }).sections.Search.find(section => section.page === 'search')?.enabled).toBe(false)
  })
})
