import type { Index, ClientSearchRequest, SearchFilter, SearchSort } from '../../api/searchkernel-client'

export function readSearchParams(params: URLSearchParams, index?: Index): ClientSearchRequest {
  return { query: params.get('q') ?? '', ranking: params.has('ranking') ? params.get('ranking')! : index?.searchDefaults.ranking,
    limit: params.has('limit') ? Number(params.get('limit')) : index?.searchDefaults.limit,
    summary: params.has('summary') ? params.get('summary')! : index?.searchDefaults.summary }
}
const operators = ['eq', 'ne', 'lt', 'lte', 'gt', 'gte', 'in', 'exists']
const keys = ['query', 'mode', 'typedQuery', 'filters', 'sorts', 'offset', 'ranking', 'limit', 'summary', 'inputs']

/** Validate URL structure only, retaining input text even when its JSON is incomplete. */
export function decodeSearchParams(params: URLSearchParams): { state: ClientSearchRequest; error?: string } {
  try {
    if (!params.has('s')) {
      const state = readSearchParams(params)
      if (state.limit !== undefined && !Number.isFinite(state.limit)) throw new Error('Invalid legacy limit')
      return { state }
    }
    const envelope = JSON.parse(params.get('s')!)
    if (!envelope || envelope.v !== 1 || !envelope.state || typeof envelope.state !== 'object' || Array.isArray(envelope.state)) throw new Error('Unsupported Search Lab URL version/shape')
    const state = envelope.state as ClientSearchRequest
    for (const key of Object.keys(state)) if (!keys.includes(key)) throw new Error(`Unknown search state field: ${key}`)
    if (typeof state.query !== 'string') throw new Error('Query must be text')
    for (const key of ['typedQuery', 'ranking', 'summary', 'inputs'] as const) if (state[key] !== undefined && typeof state[key] !== 'string') throw new Error(`${key} must be text`)
    if (state.mode !== undefined && state.mode !== 'text' && state.mode !== 'typed') throw new Error('Invalid query mode')
    for (const key of ['limit', 'offset'] as const) if (state[key] !== undefined && (typeof state[key] !== 'number' || !Number.isFinite(state[key]))) throw new Error(`${key} must be a finite number`)
    if (state.filters !== undefined && (!Array.isArray(state.filters) || state.filters.some((f: SearchFilter) => !f || typeof f.field !== 'string' || !operators.includes(f.operator) || typeof f.value !== 'string' || Object.keys(f).some(k => !['field', 'operator', 'value'].includes(k))))) throw new Error('Invalid filters')
    if (state.sorts !== undefined && (!Array.isArray(state.sorts) || state.sorts.some((s: SearchSort) => !s || typeof s.field !== 'string' || !['asc', 'desc'].includes(s.direction) || Object.keys(s).some(k => !['field', 'direction'].includes(k))))) throw new Error('Invalid sorting')
    return { state }
  } catch (error) { return { state: { query: '' }, error: `Cannot restore Search Lab URL: ${error instanceof Error ? error.message : 'Malformed state'}` } }
}

export function encodeSearchParams(state: ClientSearchRequest): URLSearchParams {
  const ordered: Record<string, unknown> = {}
  for (const key of keys as (keyof ClientSearchRequest)[]) if (state[key] !== undefined) ordered[key] = state[key]
  return new URLSearchParams({ s: JSON.stringify({ v: 1, state: ordered }) })
}
export function updateSearchState(state: ClientSearchRequest, patch: Partial<ClientSearchRequest>): ClientSearchRequest {
  const next = { ...state, ...patch }
  if (Object.keys(patch).some(key => key !== 'offset')) delete next.offset
  return next
}
