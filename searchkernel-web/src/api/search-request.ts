import { SearchRequestBuilder, parseTypedQuery, jsonInput, toCanonicalJson, queryEditorReference, q, type CanonicalJson } from '@searchkernel/client'
import { SearchKernelClientError, type ClientSearchRequest, type PreparedSearchRequest } from './searchkernel-client'

// The SDK owns query/filter construction and BFBS JSON conversion. This boundary
// maps editor controls only; it never resolves fields, defaults or query meaning.
export function buildSearchRequest(request: ClientSearchRequest) {
  const source = request.mode === 'typed' ? parseTypedQuery(JSON.parse(request.typedQuery ?? '{}')) : request.query || null
  const builder = new SearchRequestBuilder(source)
  for (const filter of request.filters ?? []) builder.filterField(filter.field, filter.operator, filter.operator === 'exists' ? undefined : jsonInput(JSON.parse(filter.value)))
  for (const sort of request.sorts ?? []) builder.sort(sort.field, sort.direction)
  if (request.limit !== undefined) builder.limit(request.limit)
  if (request.offset !== undefined) builder.offset(request.offset)
  if (request.ranking !== undefined) builder.ranking(request.ranking)
  if (request.summary !== undefined) builder.summary(request.summary)
  const inputs: unknown = JSON.parse(request.inputs ?? '{}')
  if (!inputs || typeof inputs !== 'object' || Array.isArray(inputs)) throw new TypeError('Query inputs must be a JSON object keyed by input name')
  for (const [name, item] of Object.entries(inputs)) builder.input(name, jsonInput(item))
  return builder.build()
}

// Opaque ownership ties the preview to the exact object passed to Client.search.
const prepared = new WeakMap<PreparedSearchRequest, ReturnType<typeof buildSearchRequest>>()
export function prepareSearch(request: ClientSearchRequest): PreparedSearchRequest {
  let canonical: ReturnType<typeof buildSearchRequest>
  try { canonical = buildSearchRequest(request) }
  catch (error) { throw new SearchKernelClientError(error instanceof Error ? error.message : 'Invalid request', { origin: 'client', code: 'invalid_request' }) }
  const result = Object.freeze({ request: structuredClone(request), canonical: JSON.stringify(toCanonicalJson(canonical), null, 2) })
  prepared.set(result, canonical)
  return result
}
export function preparedRequest(request: PreparedSearchRequest) {
  const canonical = prepared.get(request)
  if (!canonical) throw new TypeError('Search request must be prepared by the Search Kernel adapter')
  return canonical
}
export function typedTerm(field: string, term: string): string { return JSON.stringify(toCanonicalJson(q.term(field, term)), null, 2) }
export type { CanonicalJson }

export const typedQueryReference = () => JSON.stringify(queryEditorReference(), null, 2)
