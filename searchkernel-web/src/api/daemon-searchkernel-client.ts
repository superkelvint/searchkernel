import { prepareDocumentMutation, preparedDocumentRequest } from './document-request'
import { prepareSearch, preparedRequest } from './search-request'
import {
  Client, IndexTargetT, GrpcWebTransport, Daemon, Index as RemoteIndex, SearchKernelError, TransportError, ProtocolError,
  SearchResults, ResultDecodeError, ValueDecodeError, fromValue, toCanonicalJson, DataTypeKind, PrimitiveDataTypeKind, PrimitiveDataTypeT,
  type IndexDefinitionT, type ClientTransport,
} from '@searchkernel/client'
import {
  SearchKernelClientError, type SearchKernelClient, type Index,
  reconcileTarget, targetKey, targetName, type Target, type DaemonStatus, type FieldValue, type RequestOptions, type Capabilities,
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
  indexes: { read: true, create: true, delete: true },
  documents: { read: true, write: true, bulk: false },
  search: { basic: true, sort: true, facets: false, grouping: false },
  analysis: { text: false }, query: { inspect: false },
  schema: { read: true, write: false, changePlan: true },
  ranking: { profiles: true, features: false, write: true },
  summaries: { read: true, write: false }, metrics: { query: false, storage: false },
  admin: { overview: false, health: true, diagnostics: true },
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
  return new SearchKernelClientError(error instanceof Error ? error.message : 'Client request failed', { code: error instanceof SyntaxError ? 'invalid_argument' : 'client_error', origin: 'client' })
}

/** Owns transport/client details; React sees only the dashboard seam. */
export function createDaemonSearchKernelClient(endpoint: string, transport?: () => ClientTransport): SearchKernelClient {
  const daemon = new Daemon(endpoint, transport?.())
  type BoundIndex = Pick<RemoteIndex, 'client' | 'definition' | 'generation'>
  const handles = new Map<string, Promise<BoundIndex>>()
  const configurationClients = new Map<string, { client: Client; target: string }>()
  async function call<T>(read: () => Promise<T>): Promise<T> {
    try { return await read() } catch (error) { throw daemonError(error) }
  }
  async function opened(target: Target, options?: RequestOptions): Promise<BoundIndex> {
    if (options?.signal?.aborted) throw new DOMException('Aborted', 'AbortError')
    const key = targetKey(target)
    const existing = handles.get(key)
    if (existing) return waitForOpen(existing, options?.signal)
    // The opened Index is adapter-owned and shared across React views. Do not
    // bind its transport lifetime to one query observer's AbortSignal: StrictMode
    // or route teardown may cancel that observer while another view immediately
    // reuses the same target. Callers can still stop waiting via waitForOpen(),
    // while ordinary search/get/browse operations propagate their own signals.
    const pending = typeof target === 'string' ? RemoteIndex.open(endpoint, target, transport?.()) : (async () => {
      // A URL-bound incarnation must never OpenIndex by name and adopt a new token.
      const client = new Client(transport?.() ?? new GrpcWebTransport(endpoint), targetName(target))
      client.bindGeneration(target.generation)
      try {
        const result = await client.describe()
        if (!result.index) throw new ProtocolError('DescribeResult.index is missing')
        client.indexDefinition = result.index
        return { client, definition: result.index, generation: String(target.generation) }
      } catch (error) { await client.close(); throw error }
    })()
    // One selected target keeps one generation-bound handle. Stale-generation
    // errors are surfaced rather than silently replacing this incarnation.
    handles.set(key, pending)
    try { return await waitForOpen(pending, options?.signal) } catch (error) {
      // Only discard the shared open if the open itself failed. A caller-local
      // abort from waitForOpen must not poison a still-progressing handle.
      pending.catch(() => {
        if (handles.get(key) === pending) handles.delete(key)
      })
      throw error
    }
  }
  async function status(options?: RequestOptions): Promise<DaemonStatus> {
    try {
      const response = await fetch(`${endpoint.replace(/\/$/, '')}/admin/status`, { method: 'POST', signal: options?.signal })
      if (!response.ok) throw new SearchKernelClientError('Daemon diagnostics unavailable. Retry or check daemon readiness.', { code: 'diagnostics_unavailable', status: response.status, retryable: true, origin: 'transport' })
      const value = await response.json()
      if (typeof value.ready !== 'boolean' || typeof value.version !== 'string' || typeof value.native_runtime !== 'string' || typeof value.lifecycle !== 'string' || !Array.isArray(value.indexes) || !Array.isArray(value.unavailable) || !value.unavailable.every((item: unknown) => typeof item === 'string') || !value.indexes.every((item: any) => item && typeof item.name === 'string' && ['open', 'persisted', 'transitioning', 'unopenable'].includes(item.state) && (item.generation === null || typeof item.generation === 'string') && (item.persisted === null || typeof item.persisted === 'boolean') && (item.schema_name === null || typeof item.schema_name === 'string') && (item.warning === null || typeof item.warning === 'string'))) throw new ProtocolError('Invalid daemon status payload')
      return value
    } catch (error) { throw daemonError(error instanceof TypeError ? new TransportError('Daemon unreachable. Retry or refresh health.', error) : error) }
  }
  function retained(name: string, index: BoundIndex): Index {
    const target = new IndexTargetT(name, index.generation)
    handles.set(targetKey(target), Promise.resolve(index))
    return { ...metadata(name, index.definition), generation: index.generation, target }
  }
  return {
    daemonStatus: status,
    createIndex(name, definition) { return call(async () => retained(name, await RemoteIndex.create(endpoint, name, definition, transport?.()))) },
    planIndexChange(target, definition, options) { return call(async () => {
      // Configuration owns a separate client: ApplyIndexChange updates its
      // binding, and must never mutate a cached historical incarnation handle.
      const client = new Client(transport?.() ?? new GrpcWebTransport(endpoint), targetName(target))
      if (typeof target !== 'string') client.bindGeneration(target.generation)
      const plan = await client.planIndexChange(definition, options)
      if (!plan.definition) throw new ProtocolError('Plan result is missing the canonical definition')
      configurationClients.set(text(plan.planId), { client, target: targetKey(target) })
      return { definition: plan.definition, generation: plan.generation ? text(plan.generation) : undefined, planId: text(plan.planId), supported: plan.supported, requiresReopen: plan.requiresReopen, reasons: plan.reasons }
    }) },
    applyIndexChange(target, planId, options) { return call(async () => {
      const owner = configurationClients.get(planId)
      if (!owner || owner.target !== targetKey(target)) throw new SearchKernelClientError('Validate this exact target before applying.', { code: 'invalid_configuration_plan', origin: 'client' })
      const result = await owner.client.applyIndexChange(planId, options)
      if (!result.index) throw new ProtocolError('Apply result is missing the canonical definition')
      configurationClients.delete(planId)
      owner.client.bindGeneration(result.generation)
      owner.client.indexDefinition = result.index
      return retained(targetName(target), { client: owner.client, definition: result.index, generation: text(result.generation) })
    }) },
    reopenIndex(target, options) { return this.openIndex!(targetName(target), options) },
    openIndex(name, options) { return call(async () => retained(name, await RemoteIndex.open(endpoint, name, transport?.(), options))) },
    deleteIndex(target, options) { return call(async () => { await (await opened(target, options)).client.deleteIndex(options) }) },
    indexDiagnostics(target, options) { return call(async () => {
      const index = await opened(target, options)
      const definition = await index.client.describe(options)
      const snapshot = await status(options)
      const current = reconcileTarget(target, snapshot.indexes) === 'current' ? snapshot.indexes.find(item => item.name === target.name && item.generation === target.generation) : undefined
      if (!current) {
        // Ask the generation-bound canonical surface again to obtain a real
        // stale error. A transitioning snapshot alone cannot prove staleness.
        await index.client.describe(options)
        throw new SearchKernelClientError('Index diagnostics changed during the request. Refresh authoritative state before continuing.', { code: 'diagnostics_changed', retryable: true, origin: 'client' })
      }
      return { status: current, definition: toCanonicalJson(definition), canonical: JSON.stringify({ status: current, definition: toCanonicalJson(definition) }, null, 2) }
    }) },
    capabilities() { return call(async () => {
      const discovery = await daemon.client.listIndexes()
      const configuration = discovery.configuration
      return { ...capabilities,
        indexes: { ...capabilities.indexes, create: configuration?.createIndex ?? false },
        schema: { ...capabilities.schema, write: configuration?.fieldMutation ?? false, changePlan: configuration?.changePlan ?? false },
        ranking: { ...capabilities.ranking, write: configuration?.rankProfilesWrite ?? false },
      }
    }) },
    listIndexes(options) { return call(async () => (await daemon.listIndexes(options)).map(id => ({ id }))) },
    getIndex(target, options) { return call(async () => {
      const index = await opened(target, options)
      if (typeof target !== 'string') {
        const result = await index.client.describe(options)
        if (!result.index) throw new ProtocolError('DescribeResult.index is missing')
        index.client.indexDefinition = result.index
      }
      return retained(targetName(target), index)
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
      const result = new SearchResults(await index.client.search(preparedRequest(prepareSearch({ query: request.query ?? '', limit: request.limit })), options))
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

function waitForOpen<T>(pending: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return pending
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException('Aborted', 'AbortError'))
    signal.addEventListener('abort', abort, { once: true })
    pending.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
  })
}
