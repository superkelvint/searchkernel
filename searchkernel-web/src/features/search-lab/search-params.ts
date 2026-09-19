import type { Index, ClientSearchRequest } from '../../api/searchkernel-client'

export function readSearchParams(params: URLSearchParams, index: Index): ClientSearchRequest {
  const parsedLimit = Number(params.get('limit') ?? index.searchDefaults.limit)
  return { query: params.get('q') ?? '', ranking: index.profiles.includes(params.get('ranking') ?? '') ? params.get('ranking')! : index.searchDefaults.ranking, limit: [10, 20, 50].includes(parsedLimit) ? parsedLimit : index.searchDefaults.limit, summary: index.summaries.includes(params.get('summary') ?? '') ? params.get('summary')! : index.searchDefaults.summary }
}
