import type { Index, ClientSearchRequest } from '../../api/searchkernel-client'

export function readSearchParams(params: URLSearchParams, index: Index): ClientSearchRequest {
  // URL state carries caller intent; validation belongs to the canonical client/server.
  return {
    query: params.get('q') ?? '',
    ranking: params.has('ranking') ? params.get('ranking')! : index.searchDefaults.ranking,
    limit: params.has('limit') ? Number(params.get('limit')) : index.searchDefaults.limit,
    summary: params.has('summary') ? params.get('summary')! : index.searchDefaults.summary,
  }
}
