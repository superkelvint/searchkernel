import { prepareSearch, preparedRequest } from './search-request'
import {
  Daemon, Index as RemoteIndex, SearchKernelError, TransportError, ProtocolError,
  SearchResults, ResultDecodeError, ValueDecodeError, toCanonicalJson, DataTypeKind, PrimitiveDataTypeKind, PrimitiveDataTypeT,
  type IndexDefinitionT, type ClientTransport,
} from '@searchkernel/client'
import {
  SearchKernelClientError, type SearchKernelClient, type Index,
  type FieldValue, type RequestOptions, type Capabilities,
} from './searchkernel-client'

/** Lossless JSON-display representation for integers, byte payloads and tensors. */
export function dashboardValue(value: unknown): FieldValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value
  if (typeof value === 'bigint') return { $type: 'integer', decimal: value.toString() }
  if (typeof value === 'number') return Number.isFinite(value) ? value : { $type: 'float', value: String(value) }
  if (value instanceof Uint8Array) return { $type: 'bytes', base64: btoa(Array.from(value, byte => String.fromCharCode(byte)).join('')) }
  if (Array.isArray(value)) return value.map(dashboardValue)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, dashboardValue(item)]))
  }
  throw new SearchKernelClientError('Unsupported result value', { code: 'value_decode_error', origin: 'client' })
}

function fields(value: Record<string, unknown>): Record<string, FieldValue> {
  return Object.fromEntries(Object.entries(value).map(([name, item]) => [name, dashboardValue(item)]))
}

function text(value: string | Uint8Array | null): string {
  if (typeof value !== 'string') throw new SearchKernelClientError('Missing canonical metadata name', { code: 'protocol_error', origin: 'protocol' })
  return value
}

function metadata(target: string, definition: IndexDefinitionT): Index {
  return {
    id: target,
    profileInputs: Object.fromEntries(definition.rankProfiles.map(profile => [text(profile.name), JSON.stringify(profile.inputs.map(toCanonicalJson), null, 2)])),
    profiles: definition.rankProfiles.map(profile => text(profile.name)),
    summaries: definition.documentSummaries.map(summary => text(summary.name)),
    // Omitted controls reach the semantic resolver unchanged.
    searchDefaults: {},
    fields: definition.fields.map(field => ({
      name: text(field.name),
      type: field.dataType?.kind instanceof PrimitiveDataTypeT
        ? PrimitiveDataTypeKind[field.dataType.kind.kind].toLowerCase()
        : (DataTypeKind[field.dataType?.kindType ?? DataTypeKind.NONE] ?? 'Unknown').replace(/DataType$/, '').toLowerCase(),
      stored: field.stored?.value,
      searchable: field.searchable?.value,
      attribute: field.attribute?.enabled?.value,
      analyzer: field.analyzer?.name ? text(field.analyzer.name) : 'Unavailable',
    })),
  }
}

const capabilities: Capabilities = {
  indexes: { read: true, create: false, delete: false },
  documents: { read: true, write: false, bulk: false },
  search: { basic: true, sort: true, facets: false, grouping: false },
  analysis: { text: false }, query: { inspect: false },
  schema: { read: true, write: false, changePlan: false },
  ranking: { profiles: true, features: false, write: false },
  summaries: { read: true, write: false }, metrics: { query: false, storage: false },
  admin: { overview: false, health: false, diagnostics: false },
}

export function daemonError(error: unknown): SearchKernelClientError {
  if (error instanceof SearchKernelClientError) return error
  if (error instanceof SearchKernelError) return new SearchKernelClientError(error.message, {
    code: error.code, retryable: error.retryable, details: dashboardValue(error.details), origin: 'canonical',
  })
  if (error instanceof TransportError) {
    const cause = error.cause && typeof error.cause === 'object' ? error.cause as Record<string, unknown> : {}
    return new SearchKernelClientError(error.message, { code: error.kind === 'aborted' ? 'aborted' : 'transport_error', retryable: error.kind !== 'aborted', origin: 'transport',
      status: typeof cause.status === 'number' ? cause.status : undefined,
      details: typeof cause.code === 'number' ? { transportCode: cause.code } : undefined })
  }
  if (error instanceof ProtocolError || error instanceof ResultDecodeError || error instanceof ValueDecodeError) return new SearchKernelClientError(error.message, { code: 'protocol_error', origin: 'protocol' })
  if (error instanceof Error && error.name === 'AbortError') return new SearchKernelClientError(error.message, { code: 'aborted', origin: 'transport' })
  return new SearchKernelClientError(error instanceof Error ? error.message : 'Client request failed', { code: 'client_error', origin: 'client' })
}

/** Owns transport/client details; React sees only the dashboard seam. */
export function createDaemonSearchKernelClient(endpoint: string, transport?: () => ClientTransport): SearchKernelClient {
  const daemon = new Daemon(endpoint, transport?.())
  const handles = new Map<string, Promise<RemoteIndex>>()
  async function call<T>(read: () => Promise<T>): Promise<T> {
    try { return await read() } catch (error) { throw daemonError(error) }
  }
  async function opened(target: string, options?: RequestOptions): Promise<RemoteIndex> {
    if (options?.signal?.aborted) throw new DOMException('Aborted', 'AbortError')
    const existing = handles.get(target)
    if (existing) return waitForOpen(existing, options?.signal)
    // The opened Index is adapter-owned and shared across React views. Do not
    // bind its transport lifetime to one query observer's AbortSignal: StrictMode
    // or route teardown may cancel that observer while another view immediately
    // reuses the same target. Callers can still stop waiting via waitForOpen(),
    // while ordinary search/get/browse operations propagate their own signals.
    const pending = RemoteIndex.open(endpoint, target, transport?.())
    // One selected target keeps one generation-bound handle. Stale-generation
    // errors are surfaced rather than silently replacing this incarnation.
    handles.set(target, pending)
    try { return await waitForOpen(pending, options?.signal) } catch (error) {
      // Only discard the shared open if the open itself failed. A caller-local
      // abort from waitForOpen must not poison a still-progressing handle.
      pending.catch(() => {
        if (handles.get(target) === pending) handles.delete(target)
      })
      throw error
    }
  }
  return {
    async capabilities() { return capabilities },
    listIndexes(options) { return call(async () => (await daemon.listIndexes(options)).map(id => ({ id }))) },
    getIndex(target, options) { return call(async () => {
      const index = await opened(target, options)
      return { ...metadata(target, index.definition), generation: index.generation }
    }) },
    getDocument(target, id, options) { return call(async () => {
      const document = await (await opened(target, options)).get(id, undefined, options)
      if (document === null) return undefined
      const { _id, ...values } = document
      return { id: String(_id), fields: fields(values) }
    }) },
    browseDocuments(target, request, options) { return call(async () => {
      if (request.cursor !== undefined || request.sort !== undefined) throw new SearchKernelClientError('Browse cursor/sort is unavailable', { code: 'unsupported', origin: 'client' })
      const index = await opened(target, options)
      const builder = request.query ? index.search(request.query) : index.browse()
      if (request.limit !== undefined) builder.limit(request.limit)
      const result = await builder.execute(options)
      return { documents: result.hits.map(hit => ({ id: hit.id, fields: fields(hit.fields) })), total: result.totalHits.toString() }
    }) },
    prepareSearch,
    search(target, request, options) { return this.executeSearch(target, prepareSearch(request), options) },
    executeSearch(target, prepared, options) { return call(async () => {
      const request = prepared.request
      const index = await opened(target, options)
      const raw = await index.client.search(preparedRequest(prepared), options)
      const result = new SearchResults(raw)
      return {
        canonicalResponse: JSON.stringify(toCanonicalJson(raw), null, 2), totalRelation: result.totalHitsRelation,
        request, matches: result.totalHits.toString(),
        timingMs: result.timing?.queryTimeSeconds == null ? undefined : `${result.timing.queryTimeSeconds * 1000} ms`,
        hits: result.hits.map((hit, position) => ({ document: { id: hit.id, fields: fields(hit.fields) }, score: hit.score === null ? 'Unavailable' : String(hit.score), rank: position + 1 })),
        diagnostics: result.issues.map(issue => issue.message ?? ''),
        coverage: result.coverage ? dashboardValue(result.coverage) : undefined,
      }
    }) },
    async analyze() { throw new SearchKernelClientError('Analysis is unavailable', { code: 'unsupported' }) },
  }
}

function waitForOpen(pending: Promise<RemoteIndex>, signal?: AbortSignal): Promise<RemoteIndex> {
  if (!signal) return pending
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException('Aborted', 'AbortError'))
    signal.addEventListener('abort', abort, { once: true })
    pending.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
  })
}
