import { targetName } from './searchkernel-client'
import { prepareDocumentMutation } from './document-request'
import { prepareSearch } from './search-request'
import {
  normalizeClientError,
  type AnalysisToken,
  type AnalysisOptions,
  type Capabilities,
  type Document,
  type Field,
  type FieldValue,
  type Index,
  type RequestOptions,
  type SearchHit,
  type SearchKernelClient,
  type ClientSearchResponse,
} from './searchkernel-client'

type FixtureRecord = { id: string; score: string; fields: Record<string, FieldValue> }

const capabilities: Capabilities = {
  indexes: { read: true, create: false, delete: false },
  documents: { read: true, write: false, bulk: false },
  search: { basic: true, sort: false, facets: false, grouping: false },
  analysis: { text: true },
  query: { inspect: false },
  schema: { read: true, write: false, changePlan: false },
  ranking: { profiles: true, features: false, write: false },
  summaries: { read: true, write: false },
  metrics: { query: false, storage: false },
  admin: { overview: true, health: true, diagnostics: true },
}

const field = (name: string, type: string, options: Partial<Pick<Field, 'searchable' | 'attribute' | 'stored'>> = {}): Field => ({ name, type, searchable: options.searchable ?? false, attribute: options.attribute ?? false, stored: options.stored ?? true, analyzer: options.searchable ? 'standard-v1' : '—' })

const products: FixtureRecord[] = [
  { id: 'sku-42', score: '12.418', fields: { title: 'Wireless Headphones', brand: 'Sony', price: 149, stock: 17, category: 'Electronics' } },
  { id: 'sku-43', score: '11.884', fields: { title: 'Studio Headphones', brand: 'Bose', price: 229, stock: 4, category: 'Electronics' } },
  { id: 'sku-18', score: '9.442', fields: { title: 'QuietComfort Ultra', brand: 'Bose', price: 429, stock: 8, category: 'Electronics' } },
  { id: 'sku-91', score: '8.973', fields: { title: 'WH-1000XM5', brand: 'Sony', price: 348, stock: 12, category: 'Electronics' } },
]
const articles: FixtureRecord[] = [
  { id: 'article-1', score: '8.244', fields: { headline: 'Choosing the right headphones', section: 'Guides', published_at: '2026-09-04' } },
  { id: 'article-2', score: '7.910', fields: { headline: 'Noise cancellation explained', section: 'Guides', published_at: '2026-08-18' } },
]
const users: FixtureRecord[] = [
  { id: 'user-42', score: '5.120', fields: { display_name: 'Maya Chen', role: 'member', active: true } },
  { id: 'user-18', score: '4.901', fields: { display_name: 'Alex Rivera', role: 'member', active: true } },
]

const productFields = [field('title', 'text', { searchable: true }), field('brand', 'string', { searchable: true, attribute: true }), field('price', 'float', { attribute: true }), field('stock', 'integer', { attribute: true }), field('category', 'string', { searchable: true })]
const articleFields = [field('headline', 'text', { searchable: true }), field('section', 'string', { searchable: true, attribute: true }), field('published_at', 'date', { attribute: true })]
const userFields = [field('display_name', 'text', { searchable: true }), field('role', 'string', { searchable: true, attribute: true }), field('active', 'boolean', { attribute: true })]

const toDocument = (record: FixtureRecord): Document => ({ id: record.id, fields: record.fields })
const toHit = (record: FixtureRecord, rank: number): SearchHit => ({ document: toDocument(record), score: record.score, rank })

type FixtureIndexDefinition = { id: string; documentCount: string; size: string; latency: string; profiles: string[]; summaries: string[]; fields: Field[]; records: FixtureRecord[] }
const definitions: FixtureIndexDefinition[] = [
  { id: 'products', documentCount: '2,482,192', size: '3.8 GB', latency: '3.7 ms', profiles: ['default', 'product-search', 'semantic'], summaries: ['search-result', 'compact'], fields: productFields, records: products },
  { id: 'articles', documentCount: '81,402', size: '440 MB', latency: '2.9 ms', profiles: ['default', 'editorial'], summaries: ['search-result', 'compact'], fields: articleFields, records: articles },
  { id: 'users', documentCount: '12,832', size: '91 MB', latency: '1.8 ms', profiles: ['default', 'person-search'], summaries: ['search-result'], fields: userFields, records: users },
]
const indexes: Index[] = definitions.map(definition => ({ id: definition.id, profiles: definition.profiles, summaries: definition.summaries, searchDefaults: { ranking: definition.profiles[0]!, limit: 20, summary: definition.summaries[0]! }, fields: definition.fields }))
const searchKeys: Record<string, Record<string, FixtureRecord[]>> = { products: { '': products, 'wireless headphones': products, headphones: products, sony: [products[0]!, products[3]!], bose: [products[1]!, products[2]!] }, articles: { '': articles, headphones: articles, guides: articles, 'choosing the right headphones': [articles[0]!] }, users: { '': users, 'maya chen': [users[0]!], member: users } }
const analyses: Record<string, Record<string, AnalysisToken[]>> = { products: { 'Running with the foxes': [{ position: 0, original: 'Running', normalized: 'running', term: 'run' }, { position: 1, original: 'with', normalized: 'with', term: 'with' }, { position: 2, original: 'the', normalized: 'the', term: 'the' }, { position: 3, original: 'foxes', normalized: 'foxes', term: 'fox' }] }, articles: { 'Running with the foxes': [{ position: 0, original: 'Running', normalized: 'running', term: 'running' }, { position: 1, original: 'with', normalized: 'with', term: 'with' }, { position: 2, original: 'the', normalized: 'the', term: 'the' }, { position: 3, original: 'foxes', normalized: 'foxes', term: 'foxes' }] }, users: { 'Running with the foxes': [{ position: 0, original: 'Running', normalized: 'running', term: 'running' }, { position: 1, original: 'with', normalized: 'with', term: 'with' }, { position: 2, original: 'the', normalized: 'the', term: 'the' }, { position: 3, original: 'foxes', normalized: 'foxes', term: 'foxes' }] } }

function delay(signal?: AbortSignal) {
  return new Promise<void>((resolve, reject) => { const id = setTimeout(resolve, 160); signal?.addEventListener('abort', () => { clearTimeout(id); reject(new DOMException('Aborted', 'AbortError')) }, { once: true }) })
}

export function createFixtureSearchKernelClient(): SearchKernelClient {
  return {
    prepareDocumentMutation,
    async executeDocumentMutation() { throw new Error('Document writes require real daemon mode') },
    prepareSearch,
    executeSearch(indexId, prepared, options) {
      if (prepared.request.mode === 'typed' || prepared.request.filters?.length || prepared.request.sorts?.length || prepared.request.inputs !== undefined || prepared.request.offset) throw new Error('Advanced search requires real daemon mode')
      return this.search(indexId, prepared.request, options)
    },
    capabilities: async () => capabilities,
    listIndexes: async () => indexes,
    getIndex: async indexId => indexes.find(index => index.id === targetName(indexId)),
    getDocument: async (indexId, documentId, options: RequestOptions = {}) => {
      try {
        await delay(options.signal)
        const record = definitions.find(index => index.id === targetName(indexId))?.records.find(candidate => candidate.id === documentId)
        return record ? toDocument(record) : undefined
      } catch (error) { throw normalizeClientError(error) }
    },
    browseDocuments: async (indexId, request, options: RequestOptions = {}) => {
      try {
        await delay(options.signal)
        const definition = definitions.find(item => item.id === targetName(indexId))
        if (!definition) throw new Error(`Unknown index: ${indexId}`)
        const query = request.query?.trim().toLowerCase() ?? ''
        const records = (searchKeys[targetName(indexId)]?.[query] ?? []).slice(0, request.limit ?? 20)
        return { documents: records.map(toDocument), total: records.length }
      } catch (error) { throw normalizeClientError(error) }
    },
    async analyze(indexId, text, options: AnalysisOptions = {}) { try { await delay(options.signal); return analyses[targetName(indexId)]?.[text] ?? [] } catch (error) { throw normalizeClientError(error) } },
    async search(indexId, request, options: RequestOptions = {}): Promise<ClientSearchResponse> {
      try {
        await delay(options.signal)
        const definition = definitions.find(item => item.id === targetName(indexId))
        if (!definition) throw new Error(`Unknown index: ${indexId}`)
        const records = (searchKeys[targetName(indexId)]?.[request.query.trim().toLowerCase()] ?? []).slice(0, request.limit)
        return { request, matches: records.length, timingMs: definition.latency, hits: records.map((record, position) => toHit(record, position + 1)), resolved: `Fixture request echo { query: ${JSON.stringify(request.query)}, ranking: ${JSON.stringify(request.ranking)}, limit: ${request.limit} }`, analysis: 'No daemon linguistic analysis is available in this response.', diagnostics: ['Fixture response only', 'No daemon request executed'] }
      } catch (error) { throw normalizeClientError(error) }
    },
  }
}
