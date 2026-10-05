import { describe, expect, test } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fromCanonicalJson, fromValue, IndexTargetT } from '@searchkernel/client'
import { createFixtureSearchKernelClient } from '../../api/fixture-searchkernel-client'
import { createUnavailableAdminClient } from '../../admin-client/unavailable-admin-client'
import { ClientProvider } from '../../app/client-context'
import { IndexContext } from '../../app/index-context'
import { SearchKernelClientError, type Capabilities, type ClientSearchResponse, type Index } from '../../api/searchkernel-client'
import { CollectionNextStep, FirstSearchWelcome } from './FirstSearch'
import { canCreateCollection, collectionPath, documentPresence, hasTextFields } from './progress'
import { firstDocumentFields } from './document-fields'

const empty: ClientSearchResponse = { request: { query: '', limit: 1 }, hits: [], matches: '0', totalRelation: 'exact', diagnostics: [] }
const populated: ClientSearchResponse = { ...empty, matches: '1', hits: [{ document: { id: 'first', fields: { title: 'orchid' } }, score: '1', rank: 1 }] }

async function view(result: ClientSearchResponse, options: { guided?: boolean; write?: boolean; search?: boolean; text?: boolean; welcome?: boolean; create?: boolean; error?: boolean } = {}) {
  const client = createFixtureSearchKernelClient(), caps = await client.capabilities()
  const index = (await client.getIndex('products'))!
  index.generation = '17'; index.target = new IndexTargetT(index.id, '17')
  if (options.text === false) index.fields = index.fields.map(field => ({ ...field, searchable: false }))
  const capabilities = { ...caps, indexes: { ...caps.indexes, create: options.create ?? false }, schema: { ...caps.schema, changePlan: options.create ?? false }, documents: { ...caps.documents, write: options.write ?? true }, search: { ...caps.search, basic: options.search ?? true } }
  const cache = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  cache.setQueryData(['capabilities'], capabilities)
  cache.setQueryData(['first-search', index.id, '17'], result)
  if (options.error) await cache.fetchQuery({ queryKey: ['first-search', index.id, '17'], staleTime: 0, queryFn: () => { throw new SearchKernelClientError('The selected collection has changed.', { code: 'stale_generation', origin: 'canonical' }) } }).catch(() => undefined)
  const html = renderToStaticMarkup(<QueryClientProvider client={cache}><MemoryRouter initialEntries={[options.guided ? '/?journey=first-search' : '/']}><ClientProvider searchKernelClient={client} adminClient={createUnavailableAdminClient()}><IndexContext.Provider value={index}>{options.welcome ? <FirstSearchWelcome/> : <CollectionNextStep page="documents"/>}</IndexContext.Provider></ClientProvider></MemoryRouter></QueryClientProvider>)
  cache.clear()
  return html
}

describe('authoritative first-search progress', () => {
  test('only complete exact empty reads establish emptiness', () => {
    expect(documentPresence(empty)).toBe('empty')
    expect(documentPresence(populated)).toBe('present')
    expect(documentPresence()).toBe('unknown')
    expect(documentPresence({ ...empty, totalRelation: 'lower_bound' })).toBe('unknown')
    expect(documentPresence({ ...empty, coverage: { full: false } })).toBe('unknown')
    expect(documentPresence({ ...empty, coverage: { degraded: true } })).toBe('unknown')
    expect(documentPresence({ ...empty, coverage: { degraded: { timeout: true } } })).toBe('unknown')
    expect(documentPresence({ ...empty, coverage: { coveragePercent: 50 } })).toBe('unknown')
  })
  test('field declarations and capabilities gate possible next steps', () => {
    expect(hasTextFields({ fields: [{ type: 'string', searchable: true }] } as Index)).toBe(true)
    expect(hasTextFields({ fields: [{ type: 'tensor', searchable: true }] } as Index)).toBe(false)
    expect(canCreateCollection()).toBe(false)
    expect(canCreateCollection({ indexes: { create: true }, schema: { changePlan: false } } as Capabilities)).toBe(false)
  })
  test('all contextual links preserve logical identity and generation', () => {
    const index = { id: 'catalog', generation: '9223372036854775807' } as Index
    for (const section of ['data/documents', 'search/lab', 'configuration/schema/fields'] as const) {
      const url = new URL(collectionPath(index, section, true), 'http://localhost')
      expect(url.pathname).toBe(`/indexes/catalog/${section}`)
      expect(url.searchParams.get('generation')).toBe(index.generation)
      expect(url.searchParams.get('journey')).toBe('first-search')
    }
  })
  test('empty collection leads to writing; acknowledged data leads to Search Lab', async () => {
    expect(await view(empty, { guided: true })).toContain('Add first document')
    const ready = await view(populated, { guided: true })
    expect(ready).toContain('Try your first search')
    expect(ready).toContain('search/lab?generation=17')
    expect(ready).toContain('advanced configuration')
  })
  test('populated returning users are not forced through onboarding', async () => {
    expect(await view(populated)).toBe('')
  })
  test('cached presence cannot hide a failed incarnation check', async () => {
    const failed = await view(populated, { guided: true, error: true })
    expect(failed).toContain('stale_generation')
    expect(failed).not.toContain('Try your first search')
  })
  test('missing capabilities never offer impossible writes/search/creation', async () => {
    expect(await view(empty, { guided: true, write: false })).not.toContain('Add first document')
    expect(await view(populated, { guided: true, search: false })).not.toContain('Try your first search')
    expect(await view(empty, { guided: true, text: false })).not.toContain('Add first document')
    const welcome = await view(empty, { welcome: true, create: false })
    expect(welcome).toContain('creation is unavailable')
    expect(welcome).not.toContain('href="/indexes/new')
    // Advertising creation alone cannot compensate for missing SDK methods.
    expect(await view(empty, { welcome: true, create: true })).not.toContain('href="/indexes/new')
  })
})

test('first document entry preserves SDK scalar types and rejects lossy input', () => {
  const fields = [{ name: 'title', type: 'string' }, { name: 'count', type: 'int64' }, { name: 'price', type: 'float64' }, { name: 'active', type: 'bool' }] as Index['fields']
  const json = JSON.parse(firstDocumentFields(fields, { title: 'orchid', count: '9007199254740993', price: '1.25', active: 'false' }))
  expect(fromValue(fromCanonicalJson('Value', json.count) as Parameters<typeof fromValue>[0])).toBe(9007199254740993n)
  expect(fromValue(fromCanonicalJson('Value', json.price) as Parameters<typeof fromValue>[0])).toBe(1.25)
  expect(fromValue(fromCanonicalJson('Value', json.active) as Parameters<typeof fromValue>[0])).toBe(false)
  expect(() => firstDocumentFields(fields, { count: '1.5' })).toThrow('whole number')
  expect(() => firstDocumentFields(fields, { price: 'Infinity' })).toThrow('finite number')
  expect(() => firstDocumentFields(fields, { price: ' ' })).toThrow('finite number')
  expect(() => firstDocumentFields(fields, { active: 'maybe' })).toThrow('true or false')
  expect(() => firstDocumentFields([{ name: 'vector', type: 'tensor' }] as Index['fields'], { vector: '[1,2]' })).toThrow('advanced editor')
})
