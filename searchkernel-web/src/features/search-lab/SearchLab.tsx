import { CopyButton } from '../../components/Workbench'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { type ClientSearchRequest, type ClientSearchResponse, type PreparedSearchRequest, normalizeClientError } from '../../api/searchkernel-client'
import { typedTerm, typedQueryReference } from '../../api/search-request'
import { useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { Heading } from '../../components/Shared'
import { decodeSearchParams, encodeSearchParams, updateSearchState, canAdvancePage } from './search-params'

export function SearchLab() {
  const index = useIndex(), client = useSearchKernelClient()
  const [params, setParams] = useSearchParams()
  const key = params.toString()
  const restored = useMemo(() => decodeSearchParams(new URLSearchParams(key)), [key])
  const state = restored.state
  const prepared = useMemo(() => {
    try { return { value: client.prepareSearch(state) } }
    catch (error) { return { error: normalizeClientError(error) } }
  }, [client, state])
  const scope = JSON.stringify([index.id, index.generation])
  const [result, setResult] = useState<{ scope: string; value: ClientSearchResponse } | null>(null)
  const response = result?.scope === scope ? result.value : null
  const [error, setError] = useState<ReturnType<typeof normalizeClientError>>()
  const [loading, setLoading] = useState(false)
  const [executed, setExecuted] = useState<PreparedSearchRequest>()
  const [tab, setTab] = useState('Request')
  const [termField, setTermField] = useState(index.fields[0]?.name ?? '')
  const controller = useRef<AbortController | null>(null)
  const sequence = useRef(0)
  async function execute(request: PreparedSearchRequest) {
    controller.current?.abort()
    const current = ++sequence.current, abort = new AbortController()
    controller.current = abort
    setLoading(true); setError(undefined); setExecuted(request)
    try {
      const next = await client.executeSearch(index.id, request, { signal: abort.signal })
      if (current === sequence.current) setResult({ scope, value: next })
    } catch (failure) { if (current === sequence.current) setError(normalizeClientError(failure)) }
    finally { if (current === sequence.current) setLoading(false) }
  }
  // URL is the editor state. A valid saved URL executes once on entry. Navigation
  // reconstructs the editor; it never launches lifecycle/mutation operations.
  useEffect(() => {
    setResult(null); setError(undefined); setExecuted(undefined); setLoading(false)
    setTermField(index.fields[0]?.name ?? '')
    if (!restored.error && prepared.value && (params.has('s') || params.has('q'))) void execute(prepared.value)
    return () => { ++sequence.current; controller.current?.abort() }
    // Entry/index lifetime only: editing or navigating the URL reconstructs
    // configuration without executing. StrictMode cleanup cancels its observer;
    // the next setup executes with a fresh signal.
  }, [client, index.id, index.generation])
  const update = (patch: Partial<ClientSearchRequest>) => { const next = encodeSearchParams(updateSearchState(state, patch)); if (next.toString() !== key) setParams(next) }
  const run = () => { if (!loading && !restored.error && prepared.value) void execute(prepared.value) }
  useEffect(() => {
    const listener = (event: KeyboardEvent) => { if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') { event.preventDefault(); run() } }
    addEventListener('keydown', listener); return () => removeEventListener('keydown', listener)
  })
  const dirty = executed !== undefined && prepared.value?.canonical !== executed.canonical
  const visibleError = prepared.error ?? error
  const errorObject = (value: ReturnType<typeof normalizeClientError>) => ({ origin: value.origin, code: value.code, message: value.message, retryable: value.retryable, status: value.status, details: value.details })
  const errorJson = visibleError ? JSON.stringify(errorObject(visibleError), null, 2) : undefined
  const executionErrorJson = error ? JSON.stringify(errorObject(error), null, 2) : undefined
  const filters = state.filters ?? [], sorts = state.sorts ?? []
  const inspector = tab === 'Request' ? prepared.value?.canonical ?? errorJson ?? 'No valid request'
    : tab === 'Response' ? response?.canonicalResponse ?? 'No canonical response is available. Run a query against the daemon.'
    : tab === 'Executed request' ? executed?.canonical ?? 'No request executed yet.' : executionErrorJson ?? errorJson ?? 'No error.'
  return <div className="min-w-0">
    <Heading title="Search Lab" description="Construct, execute, inspect and share canonical Search Kernel queries."><CopyButton text={location.href} label="Copy URL" /></Heading>
    {restored.error && <div role="alert" className="surface mb-4 p-4 text-red-800">{restored.error}<button className="soft-button ml-3" onClick={() => setParams(encodeSearchParams({ query: '' }))}>Reset URL state</button></div>}
    <form className="surface flex flex-wrap gap-3 p-4" onSubmit={event => { event.preventDefault(); run() }}>
      <label className="text-sm">Query mode<select aria-label="Query mode" className="input" value={state.mode ?? 'text'} onChange={e => update({ mode: e.target.value as 'text' | 'typed', typedQuery: state.typedQuery ?? typedTerm(termField, state.query) })}><option value="text">Text</option><option value="typed">Typed JSON</option></select></label>
      <input aria-label="Search this index" className="input min-w-48 flex-1" placeholder="Enter text, or run empty text to browse" value={state.query} onChange={e => update({ query: e.target.value })} />
      <button type="submit" className="primary-button" disabled={loading || !!prepared.error || !!restored.error}>{loading ? 'Running…' : 'Search'}</button>
      {loading && <button type="button" className="soft-button" onClick={() => controller.current?.abort()}>Cancel</button>}
    </form>
    {state.mode === 'typed' && <section className="surface mt-4 space-y-3 p-4"><div className="flex flex-wrap gap-2"><label>Term field<select className="input" aria-label="Typed term field" value={termField} onChange={e => setTermField(e.target.value)}>{index.fields.map(f => <option key={f.name}>{f.name}</option>)}</select></label><button className="soft-button" onClick={() => update({ typedQuery: typedTerm(termField, state.query) })}>Use typed term</button></div><label className="block">Canonical typed query<textarea aria-label="Canonical typed query" className="input min-h-48 font-mono text-xs" value={state.typedQuery ?? ''} onChange={e => update({ typedQuery: e.target.value })} /></label><p className="text-xs text-slate-500">Standard schema-aware QuerySource JSON. Edit canonical query nodes and annotations; the daemon validates field types and supported query meaning.</p><details><summary>Typed query reference</summary><pre className="max-h-80 overflow-auto text-xs">{typedQueryReference()}</pre></details></section>}
    <details className="surface mt-4"><summary className="cursor-pointer p-4 font-semibold">Advanced controls</summary><div className="grid gap-4 px-4 pb-4 md:grid-cols-2">
      <section><h2 className="font-semibold">Filters</h2>{filters.map((filter, i) => <div key={i} className="mt-2 flex flex-wrap gap-2"><select aria-label={`Filter ${i + 1} field`} className="input w-32" value={filter.field} onChange={e => update({ filters: filters.map((f, n) => n === i ? { ...f, field: e.target.value } : f) })}>{index.fields.map(f => <option key={f.name}>{f.name}</option>)}</select><select aria-label={`Filter ${i + 1} operator`} className="input w-24" value={filter.operator} onChange={e => update({ filters: filters.map((f, n) => n === i ? { ...f, operator: e.target.value as typeof f.operator } : f) })}>{['eq', 'ne', 'lt', 'lte', 'gt', 'gte', 'in', 'exists'].map(op => <option key={op}>{op}</option>)}</select>{filter.operator !== 'exists' && <input aria-label={`Filter ${i + 1} value`} className="input w-32" value={filter.value} onChange={e => update({ filters: filters.map((f, n) => n === i ? { ...f, value: e.target.value } : f) })} placeholder="JSON value" />}<button className="soft-button" onClick={() => update({ filters: filters.filter((_, n) => n !== i) })} aria-label={`Remove filter ${i + 1}`}>Remove</button></div>)}<button className="soft-button mt-2" onClick={() => update({ filters: [...filters, { field: index.fields[0]?.name ?? '', operator: 'eq', value: 'null' }] })}>Add filter</button><p className="mt-2 text-xs text-slate-500">Multiple structured predicates are combined canonically. Values are JSON; use an array for in.</p></section>
      <section><h2 className="font-semibold">Sorting</h2>{sorts.map((sort, i) => <div key={i} className="mt-2 flex gap-2"><select aria-label={`Sort ${i + 1} field`} className="input" value={sort.field} onChange={e => update({ sorts: sorts.map((s, n) => n === i ? { ...s, field: e.target.value } : s) })}>{index.fields.map(f => <option key={f.name}>{f.name}</option>)}</select><select aria-label={`Sort ${i + 1} direction`} className="input" value={sort.direction} onChange={e => update({ sorts: sorts.map((s, n) => n === i ? { ...s, direction: e.target.value as typeof s.direction } : s) })}><option>asc</option><option>desc</option></select><button className="soft-button" aria-label={`Move sort ${i + 1} up`} disabled={i === 0} onClick={() => { const next = [...sorts]; [next[i - 1], next[i]] = [next[i], next[i - 1]]; update({ sorts: next }) }}>↑</button><button className="soft-button" aria-label={`Remove sort ${i + 1}`} onClick={() => update({ sorts: sorts.filter((_, n) => n !== i) })}>Remove</button></div>)}<button className="soft-button mt-2" onClick={() => update({ sorts: [...sorts, { field: index.fields[0]?.name ?? '', direction: 'asc' }] })}>Add sort</button></section>
      <label>Ranking profile<input aria-label="Ranking profile" list="rank-profiles" className="input" placeholder="Server default (omitted)" value={state.ranking ?? ''} onChange={e => update({ ranking: e.target.value || undefined })} /><datalist id="rank-profiles">{index.profiles.map(profile => <option key={profile}>{profile}</option>)}</datalist></label>
      <label>Summary<input aria-label="Summary" list="summaries" className="input" placeholder="Server default (omitted)" value={state.summary ?? ''} onChange={e => update({ summary: e.target.value || undefined })} /><datalist id="summaries">{index.summaries.map(summary => <option key={summary}>{summary}</option>)}</datalist></label>
      <label>Limit<input aria-label="Limit" type="number" className="input" placeholder="Server default" value={state.limit ?? ''} onChange={e => update({ limit: e.target.value === '' ? undefined : Number(e.target.value) })} /></label>
      <label>Offset<input aria-label="Offset" type="number" className="input" placeholder="Server default" value={state.offset ?? ''} onChange={e => update({ offset: e.target.value === '' ? undefined : Number(e.target.value) })} /></label>
      <label className="md:col-span-2">Query inputs<textarea aria-label="Query inputs" className="input min-h-24 font-mono text-xs" value={state.inputs ?? ''} placeholder='{"boost": 2}' onChange={e => update({ inputs: e.target.value || undefined })} /><p className="mt-2 text-xs text-slate-500">Literal inputs: numeric scalars and dense tensors execute in this daemon. JSON object keyed by input name. Scalar/container values retain their types; int64, uint64 and tensors can use canonical Value JSON. Expression inputs require a container-aware backend and are unavailable here; other unsupported literal types retain canonical errors.</p>{state.ranking && index.profileInputs?.[state.ranking] && <details><summary className="text-sm">Declared profile inputs</summary><pre className="overflow-auto text-xs">{index.profileInputs[state.ranking]}</pre></details>}</label>
    </div></details>
    {visibleError && <div role="alert" className="surface mt-4 border-red-200 p-4 text-red-800"><strong>{visibleError.origin ?? 'client'} · {visibleError.code}</strong><p>{visibleError.message}</p><details><summary>Structured error</summary><pre className="overflow-auto text-xs">{errorJson}</pre></details></div>}
    <div className="mt-4 grid gap-4 xl:grid-cols-2">
      <section className="surface min-w-0 p-4" aria-label="Search results"><h2 className="font-semibold">Results</h2>{loading && <p role="status">Running request… Previous results are retained.</p>}{error && response && <p>Previous successful response retained; the latest request failed.</p>}{dirty && <p className="text-amber-700">Editor differs from the last executed request.</p>}<p>{response ? `${response.matches} matches` : executed ? 'No successful response yet.' : 'No query executed. Enter a query, or run an empty text query to browse.'} {response?.totalRelation} {response?.timingMs}</p>{response && !response.hits.length && <p>{String(response.matches) === '0' ? 'No hits matched this query.' : 'This page returned no hits.'}</p>}{response?.hits.map(hit => <article key={hit.document.id} className="mt-3 border-t py-3"><div className="flex justify-between"><strong>{hit.document.id}</strong><code>Score: {hit.score}</code></div><pre className="mt-2 overflow-auto whitespace-pre-wrap text-xs">{JSON.stringify(hit.document.fields, null, 2)}</pre></article>)}<div className="mt-4 flex flex-wrap gap-2"><button className="soft-button" disabled={loading || !!prepared.error || !!restored.error || dirty || !!error || state.limit === undefined || !state.offset} onClick={() => { const next = { ...state, offset: Math.max(0, (state.offset ?? 0) - state.limit!) }; setParams(encodeSearchParams(next)); try { void execute(client.prepareSearch(next)) } catch (e) { setError(normalizeClientError(e)) } }}>Previous page</button><button className="soft-button" disabled={loading || !!prepared.error || !!restored.error || dirty || !!error || !canAdvancePage(state, response)} onClick={() => { const next = { ...state, offset: (state.offset ?? 0) + state.limit! }; setParams(encodeSearchParams(next)); try { void execute(client.prepareSearch(next)) } catch (e) { setError(normalizeClientError(e)) } }}>Next page</button></div>{state.limit === undefined && <p className="mt-2 text-xs">Set an explicit limit to use page controls.</p>}</section>
      <section className="surface min-w-0 p-4"><div className="flex flex-wrap gap-2" role="tablist">{['Request', 'Response', 'Executed request', 'Error'].map(name => <button key={name} role="tab" aria-selected={name === tab} className="soft-button" onClick={() => setTab(name)}>{name}</button>)}</div><p className="my-2 text-xs">{tab === 'Response' ? 'SearchResult payload' : tab === 'Error' ? 'Search Kernel error' : 'SearchRequest payload'} · target {index.id} · generation {index.generation ?? 'unavailable'}</p><CopyButton text={inspector} label={`Copy ${tab}`} /><pre aria-label={`${tab} inspector`} className="mt-3 max-h-150 overflow-auto rounded bg-slate-950 p-4 font-mono text-xs text-slate-100">{inspector}</pre></section>
    </div>
  </div>
}
