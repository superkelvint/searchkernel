import type { Capabilities, ClientSearchResponse, Index } from '../../api/searchkernel-client'

// A bounded match-all read establishes presence, never a fabricated count.
export function documentPresence(result?: ClientSearchResponse): 'empty' | 'present' | 'unknown' {
  if (!result) return 'unknown'
  if (result.hits.length) return 'present'
  const coverage = result.coverage
  const incomplete = coverage && typeof coverage === 'object' && !Array.isArray(coverage) && (coverage.full === false || coverage.degraded === true)
  return !incomplete && String(result.matches) === '0' && result.totalRelation === 'exact' ? 'empty' : 'unknown'
}
export function hasTextFields(index: Index) {
  return index.fields.some(field => field.searchable === true && ['string', 'text'].includes(field.type))
}
export function canCreateCollection(capabilities?: Capabilities) {
  return capabilities?.indexes.create === true && capabilities.schema.changePlan === true
}
export function collectionPath(index: Index, section: 'data/documents' | 'search/lab' | 'configuration/schema/fields', guided = false) {
  const params = new URLSearchParams()
  if (index.generation) params.set('generation', index.generation)
  if (guided) params.set('journey', 'first-search')
  return `/indexes/${encodeURIComponent(index.id)}/${section}?${params}`
}
