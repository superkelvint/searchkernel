/**
 * Browser-facing shape of the generated Search Kernel client.
 *
 * The dashboard depends on this contract, not on a transport or on fixture
 * storage. The fixture implementation is only one provider of this client.
 */
export type Capabilities = {
  indexes: { read: boolean; create: boolean; delete: boolean }
  documents: { read: boolean; write: boolean; bulk: boolean }
  search: { basic: boolean; sort: boolean; facets: boolean; grouping: boolean }
  analysis: { text: boolean }
  query: { inspect: boolean }
  schema: { read: boolean; write: boolean; changePlan: boolean }
  ranking: { profiles: boolean; features: boolean; write: boolean }
  summaries: { read: boolean; write: boolean }
  metrics: { query: boolean; storage: boolean }
  admin: { overview: boolean; health: boolean; diagnostics: boolean }
}

export type FieldValue = string | number | boolean | null | FieldValue[] | { [key: string]: FieldValue }

export type Document = {
  id: string
  fields: Record<string, FieldValue>
  canonical?: string
  canonicalFields?: string
}

export type Field = {
  name: string
  type: string
  searchable?: boolean
  attribute?: boolean
  stored?: boolean
  analyzer: string
}

export type Index = {
  id: string
  generation?: string
  profileInputs?: Record<string, string>
  profiles: string[]
  summaries: string[]
  searchDefaults: SearchRequestDefaults
  fields: Field[]
}

export type SearchFilter = { field: string; operator: 'eq' | 'ne' | 'lt' | 'lte' | 'gt' | 'gte' | 'in' | 'exists'; value: string }
export type SearchSort = { field: string; direction: 'asc' | 'desc' }
export type ClientSearchRequest = { query: string; mode?: 'text' | 'typed'; typedQuery?: string; filters?: SearchFilter[]; sorts?: SearchSort[]; offset?: number; ranking?: string; limit?: number; summary?: string; inputs?: string }
export type PreparedSearchRequest = { readonly request: ClientSearchRequest; readonly canonical: string }
export type DocumentMutationOperation = 'insert' | 'upsert' | 'patch' | 'update' | 'delete'
export type DocumentEditorState = { operation: DocumentMutationOperation; id: string; fields: string; operations: string; condition?: string; create?: boolean }
export type PreparedDocumentMutation = { readonly operation: DocumentMutationOperation; readonly id: string; readonly canonical: string }
export type DocumentMutationResult = { operation: DocumentMutationOperation; id: string; canonicalResponse: string; deleted?: boolean }
export type SearchRequestDefaults = Pick<ClientSearchRequest, 'ranking' | 'limit' | 'summary'>
export type BrowseDocumentsRequest = { query?: string; cursor?: string; limit?: number; sort?: string }
export type BrowseDocumentsResponse = { documents: Document[]; nextCursor?: string; total?: number | string }
export type SearchHit = { document: Document; score: string; rank: number }
export type ClientSearchResponse = {
  request: ClientSearchRequest
  matches: number | string
  timingMs?: string
  hits: SearchHit[]
  resolved?: string
  analysis?: string
  diagnostics: string[]
  canonicalResponse?: string
  totalRelation?: string
  coverage?: FieldValue
}
export type AnalysisToken = { position: number; original: string; normalized: string; term: string }
export type RequestOptions = { signal?: AbortSignal }
export type AnalysisOptions = RequestOptions & { field?: string }

export type ClientErrorDetails = FieldValue
export class SearchKernelClientError extends Error {
  readonly code: string
  readonly status?: number
  readonly retryable: boolean
  readonly details?: ClientErrorDetails
  readonly origin?: 'canonical' | 'transport' | 'protocol' | 'client'

  constructor(message: string, options: { code?: string; status?: number; retryable?: boolean; details?: ClientErrorDetails; origin?: SearchKernelClientError['origin'] } = {}) {
    super(message)
    this.name = 'SearchKernelClientError'
    this.code = options.code ?? 'client_error'
    this.status = options.status
    this.retryable = options.retryable ?? false
    this.details = options.details
    this.origin = options.origin
  }
}

export function normalizeClientError(error: unknown): SearchKernelClientError {
  if (error instanceof SearchKernelClientError) return error
  if (typeof DOMException !== 'undefined' && error instanceof DOMException && error.name === 'AbortError') return new SearchKernelClientError('The request was cancelled.', { code: 'aborted' })
  if (error instanceof Error) {
    const candidate = error as Error & Partial<Omit<SearchKernelClientError, 'name' | 'message'>>
    return new SearchKernelClientError(error.message, { code: candidate.code ?? 'unknown_error', status: candidate.status, retryable: candidate.retryable ?? true, details: candidate.details })
  }
  if (typeof error === 'object' && error !== null) {
    const candidate = error as { code?: unknown; message?: unknown; status?: unknown; retryable?: unknown; details?: unknown }
    const details = typeof candidate.details === 'object' && candidate.details !== null ? candidate.details as ClientErrorDetails : undefined
    return new SearchKernelClientError(typeof candidate.message === 'string' ? candidate.message : 'The Search Kernel request failed.', { code: typeof candidate.code === 'string' ? candidate.code : 'unknown_error', status: typeof candidate.status === 'number' ? candidate.status : undefined, retryable: typeof candidate.retryable === 'boolean' ? candidate.retryable : true, details })
  }
  return new SearchKernelClientError('The Search Kernel request failed.', { code: 'unknown_error', retryable: true })
}

export interface SearchKernelClient {
  capabilities(): Promise<Capabilities>
  listIndexes(options?: RequestOptions): Promise<Array<{ id: string }>>
  getIndex(indexId: string, options?: RequestOptions): Promise<Index | undefined>
  getDocument(indexId: string, documentId: string, options?: RequestOptions): Promise<Document | undefined>
  browseDocuments(indexId: string, request: BrowseDocumentsRequest, options?: RequestOptions): Promise<BrowseDocumentsResponse>
  prepareDocumentMutation(state: DocumentEditorState): PreparedDocumentMutation
  executeDocumentMutation(indexId: string, request: PreparedDocumentMutation, options?: RequestOptions): Promise<DocumentMutationResult>
  prepareSearch(request: ClientSearchRequest): PreparedSearchRequest
  executeSearch(indexId: string, request: PreparedSearchRequest, options?: RequestOptions): Promise<ClientSearchResponse>
  search(indexId: string, request: ClientSearchRequest, options?: RequestOptions): Promise<ClientSearchResponse>
  analyze(indexId: string, text: string, options?: AnalysisOptions): Promise<AnalysisToken[]>
}

export const unsupportedCapabilityMessage = 'Unavailable because this capability is not exposed by the server.'
