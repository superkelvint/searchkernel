import { initialSearchRequest, type Index, type ClientSearchRequest } from '../../api/searchkernel-client'

export function readSearchParams(params: URLSearchParams, index: Index): ClientSearchRequest {
  const parsedLimit = Number(params.get('limit') ?? initialSearchRequest.limit)
  return { ...initialSearchRequest, query: params.get('q') ?? '', ranking: index.profiles.includes(params.get('ranking') ?? '') ? params.get('ranking')! : index.profiles[0]!, limit: [10, 20, 50].includes(parsedLimit) ? parsedLimit : initialSearchRequest.limit, summary: index.summaries.includes(params.get('summary') ?? '') ? params.get('summary')! : index.summaries[0]! }
}
