import { prepareDocumentMutation, preparedDocumentRequest } from './document-request'
import { prepareSearch, preparedRequest } from './search-request'
import { prepareDiagnostics, preparedDiagnosticRequest, diagnosticCurl } from './diagnostic-request'
import { diagnosticResponse } from './diagnostic-response'
import {
  Client, GrpcWebTransport, Daemon, Index as RemoteIndex, SearchKernelError, TransportError, ProtocolError,
  SearchResults, ResultDecodeError, ValueDecodeError, fromValue, toCanonicalJson, DataTypeKind, PrimitiveDataTypeKind, PrimitiveDataTypeT,
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
    definition,
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
  documents: { read: true, write: true, bulk: false },
  search: { basic: true, sort: true, facets: false, grouping: false },
  analysis: { text: false }, query: { inspect: false },
  schema: { read: true, write: false, changePlan: true },
  ranking: { profiles: true, features: false, write: true },
  summaries: { read: true, write: false }, metrics: { query: false, storage: false },
  admin: { overview: false, health: false, diagnostics: false },
}

export function daemonError(error: unknown): SearchKernelClientError {
  if (error instanceof SearchKernelClientError) return error
  if (error instanceof SearchKernelError) return new SearchKernelClientError(error.message, {
    code: error.code, retryable: error.retryable, details: error.details.map(detail => dashboardValue(toCanonicalJson(detail))), origin: 'canonical',
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
  const configurationClients = new Map<string, Client>()
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
    prepareDiagnostics,
    diagnosticsCurl: diagnosticCurl,
    executeDiagnostics(prepared, options) { return call(async () => {
      const index = await opened(prepared.target, options)
      if (index.generation !== prepared.generation) throw new SearchKernelClientError(
        'The index is no longer the same incarnation. Refresh/reselect it explicitly before collecting evidence.',
        { code: 'stale_index_handle', origin: 'client', retryable: false })
      const raw = await index.queryDiagnostics(preparedDiagnosticRequest(prepared), options)
      return diagnosticResponse(raw, dashboardValue)
    }) },
    planIndexChange(target, definition, options) { return call(async () => {
      let client = configurationClients.get(target)
      if (!client) {
        client = handles.has(target) ? (await opened(target, options)).client : new Client(transport?.() ?? new GrpcWebTransport(endpoint), target)
        configurationClients.set(target, client)
      }
      const plan = await client.planIndexChange(definition, options)
      if (!plan.definition) throw new ProtocolError('Plan result is missing the canonical definition')
      return { definition: plan.definition, generation: plan.generation ? text(plan.generation) : undefined,
        planId: text(plan.planId), supported: plan.supported, requiresReopen: plan.requiresReopen, reasons: plan.reasons }
    }) },
    applyIndexChange(target, planId, options) { return call(async () => {
      const client = configurationClients.get(target)
      if (!client) throw new SearchKernelClientError('Validate this draft before applying it.', { code: 'invalid_configuration_plan', origin: 'client' })
      const result = await client.applyIndexChange(planId, options)
      if (!result.index) throw new ProtocolError('Apply result is missing the canonical definition')
      configurationClients.delete(target)
      return { ...metadata(target, result.index), generation: text(result.generation) }
    }) },
    reopenIndex(target, options) { return call(async () => {
      const previous = handles.get(target)
      handles.delete(target)
      configurationClients.delete(target)
      if (previous) await (await previous).client.close()
      const index = await opened(target, options)
      return { ...metadata(target, index.definition), generation: index.generation }
    }) },
    capabilities() { return call(async () => {
      const discovery = await daemon.client.listIndexes()
      const configuration = discovery.configuration
      return { ...capabilities,
        query: { ...capabilities.query, diagnostics: discovery.queryDiagnostics },
        indexes: { ...capabilities.indexes, create: configuration?.createIndex ?? false },
        schema: { ...capabilities.schema, write: configuration?.fieldMutation ?? false, changePlan: configuration?.changePlan ?? false },
        ranking: { ...capabilities.ranking, write: configuration?.rankProfilesWrite ?? false },
      }
    }) },
    listIndexes(options) { return call(async () => (await daemon.listIndexes(options)).map(id => ({ id }))) },
    getIndex(target, options) { return call(async () => {
      const index = await opened(target, options)
      return { ...metadata(target, index.definition), generation: index.generation }
    }) },
    getDocument(target, id, options) { return call(async () => {
      const raw = await (await opened(target, options)).client.get(id, undefined, options)
      if (raw.document === null) return undefined
      const values = Object.fromEntries(raw.document.fields.map(entry => [text(entry.key), fromValue(entry.value!)]))
      const canonicalFields = Object.fromEntries(raw.document.fields.map(entry => [text(entry.key), toCanonicalJson(entry.value)]))
      return { id: text(raw.document.id), fields: fields(values), canonical: JSON.stringify(toCanonicalJson(raw), null, 2), canonicalFields: JSON.stringify(canonicalFields, null, 2) }
    }) },
    browseDocuments(target, request, options) { return call(async () => {
      if (request.cursor !== undefined || request.sort !== undefined) throw new SearchKernelClientError('Browse cursor/sort is unavailable', { code: 'unsupported', origin: 'client' })
      const index = await opened(target, options)
      const builder = request.query ? index.search(request.query) : index.browse()
      if (request.limit !== undefined) builder.limit(request.limit)
      const result = await builder.execute(options)
      return { documents: result.hits.map(hit => ({ id: hit.id, fields: fields(hit.fields) })), total: result.totalHits.toString() }
    }) },
    prepareDocumentMutation,
    executeDocumentMutation(target, prepared, options) { return call(async () => {
      const index = await opened(target, options)
      const raw = await index.client.execute(prepared.operation, preparedDocumentRequest(prepared), options)
      if (text(raw.id) !== prepared.id) throw new ProtocolError('Mutation response document ID differs from its request')
      return { operation: prepared.operation, id: text(raw.id), canonicalResponse: JSON.stringify(toCanonicalJson(raw), null, 2), deleted: 'deleted' in raw ? raw.deleted : undefined }
    }) },
    prepareSearch,
    search(target, request, options) { return this.executeSearch(target, prepareSearch(request), options) },
    executeSearch(target, prepared, options) { return call(async () => {
      const request = prepared.request
      const index = await opened(target, options)
      const raw = await index.client.search(preparedRequest(prepared), options)
      const result = new SearchResults(raw)
      return {
        sourceRunId: raw.metadata.find(entry => entry.key === 'searchkernel.source_run')?.value
          ? String(fromValue(raw.metadata.find(entry => entry.key === 'searchkernel.source_run')!.value!)) : undefined,
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
