import {
  Daemon, Index as RemoteIndex, SearchKernelError, TransportError, ProtocolError,
  DataTypeKind, PrimitiveDataTypeKind, PrimitiveDataTypeT,
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
    profiles: definition.rankProfiles.map(profile => text(profile.name)),
    summaries: definition.documentSummaries.map(summary => text(summary.name)),
    // Omitted controls reach the semantic resolver unchanged.
    searchDefaults: {},
    fields: definition.fields.map(field => ({
      name: text(field.name),
      type: field.dataType?.kind instanceof PrimitiveDataTypeT
        ? PrimitiveDataTypeKind[field.dataType.kind.kind].toLowerCase()
        : (DataTypeKind[field.dataType?.kindType ?? DataTypeKind.NONE] ?? 'Unknown').replace(/DataType$/, '').toLowerCase(),
      stored: field.stored?.value === true,
      searchable: field.searchable?.value === true,
      attribute: field.attribute !== null,
      analyzer: field.analyzer?.name ? text(field.analyzer.name) : 'Unavailable',
    })),
  }
}

const capabilities: Capabilities = {
  indexes: { read: true, create: false, delete: false },
  documents: { read: true, write: false, bulk: false },
  search: { basic: true, sort: false, facets: false, grouping: false },
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
  if (error instanceof TransportError) return new SearchKernelClientError(error.message, { code: error.kind === 'aborted' ? 'aborted' : 'transport_error', retryable: error.kind !== 'aborted', origin: 'transport' })
  if (error instanceof ProtocolError) return new SearchKernelClientError(error.message, { code: 'protocol_error', origin: 'protocol' })
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
  async function opened(target: string, options?: RequestOptions, refresh = false): Promise<RemoteIndex> {
    if (options?.signal?.aborted) throw new DOMException('Aborted', 'AbortError')
    const existing = handles.get(target)
    if (existing && !refresh) return waitForOpen(existing, options?.signal)
    const pending = RemoteIndex.open(endpoint, target, transport?.(), options)
    // Never rebind or retry on stale-generation errors. Only an explicit
    // workspace metadata read (getIndex) can open a fresh incarnation.
    handles.set(target, pending)
    try { return await pending } catch (error) {
      if (handles.get(target) === pending) handles.delete(target)
      throw error
    }
  }
  return {
    async capabilities() { return capabilities },
    listIndexes(options) { return call(async () => (await daemon.listIndexes(options)).map(id => ({ id }))) },
    getIndex(target, options) { return call(async () => metadata(target, (await opened(target, options, true)).definition)) },
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
    search(target, request, options) { return call(async () => {
      const index = await opened(target, options)
      const builder = request.query ? index.search(request.query) : index.browse()
      if (request.limit !== undefined) builder.limit(request.limit)
      if (request.ranking !== undefined) builder.ranking(request.ranking)
      if (request.summary !== undefined) builder.summary(request.summary)
      const result = await builder.execute(options)
      return {
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
