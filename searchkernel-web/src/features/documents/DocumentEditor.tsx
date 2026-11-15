import { firstDocumentFields, simpleField } from '../onboarding/document-fields'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { normalizeClientError, SearchKernelClientError, type Document, type DocumentEditorState, type PreparedSearchRequest } from '../../api/searchkernel-client'
import { mutationOperation } from '../../api/document-request'
import { CanonicalInspector, ErrorNotice } from '../../components/Workbench'
import { mutateAndVerify } from './workflow'

export function DocumentEditor({ document, id = '', browse, open, guided = false }: { document?: Document; id?: string; guided?: boolean; browse: PreparedSearchRequest; open: (id: string) => void }) {
  const index = useIndex(), client = useSearchKernelClient(), cache = useQueryClient()
  const [state, setState] = useState<DocumentEditorState>({ operation: id ? 'update' : 'insert', id, fields: document?.canonicalFields ?? '{}', operations: '[]' })
  const [advanced, setAdvanced] = useState(!guided || !index.fields.every(simpleField))
  const [inputs, setInputs] = useState<Record<string, string>>({})
  const basic = guided && !advanced
  const [field, setField] = useState(index.fields[0]?.name ?? '')
  const [kind, setKind] = useState('assign'), [value, setValue] = useState('null')
  const [confirmed, setConfirmed] = useState(false)
  const [running, setRunning] = useState(false), [error, setError] = useState<unknown>()
  const [result, setResult] = useState<Awaited<ReturnType<typeof mutateAndVerify>>>()
  const [unknownMutation, setUnknownMutation] = useState<{ id: string; operation: DocumentEditorState['operation'] }>()
  const [reconciliation, setReconciliation] = useState<Document | null>()
  const [reconciling, setReconciling] = useState(false)
  const active = useRef(false), live = useRef(true), readAbort = useRef<AbortController | null>(null)
  useEffect(() => { live.current = true; return () => { live.current = false; readAbort.current?.abort() } }, [])
  const prepared = useMemo(() => { try { return { value: client.prepareDocumentMutation(basic ? { ...state, fields: firstDocumentFields(index.fields, inputs) } : state) } } catch (error) { return { error } } }, [client, state, basic, index.fields, inputs])
  const update = (patch: Partial<DocumentEditorState>) => { setState(previous => ({ ...previous, ...patch })); setConfirmed(false); setError(undefined) }
  async function submit() {
    if (active.current || !prepared.value || (state.operation === 'delete' && !confirmed)) return
    active.current = true; setRunning(true); setError(undefined); setResult(undefined); setReconciliation(undefined)
    const controller = new AbortController(); readAbort.current = controller
    try {
      // Abort cached reads before the write; consumed signals prevent an older
      // in-flight GET/search from overwriting the post-publication refetch.
      await Promise.all([cache.cancelQueries({ queryKey: ['document', index.id, index.generation] }), cache.cancelQueries({ queryKey: ['document-browse', index.id, index.generation] }), cache.cancelQueries({ queryKey: ['first-search', index.id, index.generation] })])
      const next = await mutateAndVerify(client, index.target ?? index.id, prepared.value, browse, { signal: controller.signal })
      await Promise.all([cache.invalidateQueries({ queryKey: ['document', index.id, index.generation] }), cache.invalidateQueries({ queryKey: ['document-browse', index.id, index.generation] }), cache.invalidateQueries({ queryKey: ['first-search', index.id, index.generation] })])
      if (live.current) { setResult(next); if (next.document?.canonicalFields) setState(previous => ({ ...previous, fields: next.document!.canonicalFields! })) }
    } catch (error) {
      if (live.current) {
        const normalized = normalizeClientError(error)
        if (normalized.code === 'mutation_outcome_unknown') setUnknownMutation({ id: prepared.value.id, operation: prepared.value.operation })
        setError(normalized)
      }
    }
    finally { active.current = false; if (live.current) setRunning(false) }
  }
  async function reconcileOutcome() {
    if (!unknownMutation || reconciling) return
    setReconciling(true); setError(undefined)
    const controller = new AbortController(); readAbort.current = controller
    try {
      const fresh = await client.getDocument(index.target ?? index.id, unknownMutation.id, { signal: controller.signal })
      await Promise.all([cache.invalidateQueries({ queryKey: ['document', index.id, index.generation] }), cache.invalidateQueries({ queryKey: ['document-browse', index.id, index.generation] }), cache.invalidateQueries({ queryKey: ['first-search', index.id, index.generation] })])
      if (live.current) {
        setReconciliation(fresh ?? null)
        setUnknownMutation(undefined)
        if (fresh?.canonicalFields && state.id === unknownMutation.id) setState(previous => ({ ...previous, fields: fresh.canonicalFields! }))
      }
    } catch (error) { if (live.current) setError(normalizeClientError(error)) }
    finally { if (live.current) setReconciling(false) }
  }
  const simple = state.operation === 'insert' || state.operation === 'upsert'
  return <section className="surface mt-4 min-w-0 space-y-3 p-4" aria-label="Document mutation editor">
    <h3 className="font-semibold">{basic ? "Add a document" : "Document mutation"}</h3>
    {guided && !index.fields.every(simpleField) && <p>This collection includes structured fields. Use the advanced editor below to preserve their types; the request inspector shows exactly what will be saved.</p>}
    {basic && <p>Give this document a unique ID and add some text. Saving sends it to Search Kernel and checks it with fresh reads.</p>}
    <fieldset disabled={running || reconciling || !!unknownMutation} className="space-y-3">
      <div className="flex flex-wrap gap-3">
        {!basic && <label>Operation<select className="input" aria-label="Document operation" value={state.operation} onChange={e => { update({ operation: e.target.value as DocumentEditorState['operation'] }); setKind('assign') }}>{['insert', 'upsert', 'patch', 'update', 'delete'].map(op => <option key={op}>{op}</option>)}</select></label>}
        <label className="min-w-40 flex-1">Document ID<input aria-label="Mutation document ID" className="input" value={state.id} onChange={e => update({ id: e.target.value })}/></label>
      </div>
      {basic && <>{index.fields.map(field => <label key={field.name} className="block">{field.name}{field.searchable && <span className="muted"> · searchable text</span>}{field.type === 'bool' ? <select aria-label={`Document field ${field.name}`} className="input" value={inputs[field.name] ?? ''} onChange={e => setInputs(previous => ({ ...previous, [field.name]: e.target.value }))}><option value="">Not supplied</option><option value="true">True</option><option value="false">False</option></select> : <input aria-label={`Document field ${field.name}`} className="input" inputMode={['int64', 'int'].includes(field.type) ? 'numeric' : ['float64', 'float'].includes(field.type) ? 'decimal' : 'text'} value={inputs[field.name] ?? ''} onChange={e => setInputs(previous => ({ ...previous, [field.name]: e.target.value }))}/>}</label>)}<button className="soft-button" onClick={() => { try { update({ fields: firstDocumentFields(index.fields, inputs) }); setAdvanced(true) } catch (error) { setError(error) } }}>Use advanced document editor</button></>}
      {simple && !basic && <><p className="text-xs text-slate-500">{state.operation === 'insert' ? 'Create only: an existing ID returns a conflict.' : 'Atomic backend put: creates or replaces the complete document. Omitted fields are not preserved.'}</p><label className="block">Fields JSON<textarea aria-label="Document fields" className="input min-h-40 font-mono text-xs" value={state.fields} onChange={e => update({ fields: e.target.value })}/></label><p className="text-xs text-slate-500">Ordinary JSON scalars/containers, or standard canonical Value objects per field for lossless int64/uint64 and tensors. GET-prefilled values preserve their canonical types.</p></>}
      {(state.operation === 'patch' || state.operation === 'update') && <>
        <div className="flex flex-wrap gap-2">
          <select aria-label="Mutation field" className="input w-40" value={field} onChange={e => setField(e.target.value)}>{index.fields.map(f => <option key={f.name}>{f.name}</option>)}</select>
          <select aria-label="Update kind" className="input w-40" value={kind} onChange={e => setKind(e.target.value)}>{(state.operation === 'patch' ? ['assign', 'clear'] : ['assign', 'clear', 'increment', 'decrement', 'multiply', 'divide']).map(op => <option key={op}>{op}</option>)}</select>
          {kind !== 'clear' && <input aria-label="Update value" className="input min-w-40 flex-1" value={value} onChange={e => setValue(e.target.value)}/>}
          <button type="button" className="soft-button" onClick={() => { try { const operations = JSON.parse(state.operations); if (!Array.isArray(operations)) throw new TypeError('Operations must be an array'); update({ operations: JSON.stringify([...operations, mutationOperation(state.operation as 'patch' | 'update', field, kind, value)], null, 2) }) } catch (error) { setError(new SearchKernelClientError(error instanceof Error ? error.message : 'Malformed update value', { code: 'invalid_request', origin: 'client' })) } }}>Add operation</button>
        </div>
        <label className="block">Canonical operations JSON<textarea aria-label="Document operations" className="input min-h-40 font-mono text-xs" value={state.operations} onChange={e => update({ operations: e.target.value })}/></label>
        <p className="text-xs text-slate-500">Partial updates preserve other fields. Assignment, clear and arithmetic execute on simple fields; assignment also supports complete dense tensors. Collection add/remove/match, addressed tensor modify and nested paths currently return canonical unsupported errors.</p>
        {state.operation === 'update' && <label><input type="checkbox" aria-label="Create missing document" checked={state.create === true} onChange={e => update({ create: e.target.checked ? true : undefined })}/> Create missing document before update</label>}
      </>}
      {state.operation === 'delete' && <label className="block text-red-800"><input type="checkbox" aria-label="Confirm document deletion" checked={confirmed} onChange={e => setConfirmed(e.target.checked)}/> Confirm deletion of <code>{state.id}</code> from <code>{index.id}</code>.</label>}
    </fieldset>
    <ErrorNotice error={error ?? (basic && !state.id ? undefined : prepared.error)}/>
    {unknownMutation && <div role="status" className="surface border-amber-200 p-3"><p>The {unknownMutation.operation} response was lost, so whether the write committed is unknown. Do not repeat it until the current document has been read from Search Kernel.</p><button className="soft-button mt-2" disabled={reconciling} onClick={() => { void reconcileOutcome() }}>{reconciling ? 'Reading current document…' : 'Read current document before retry'}</button></div>}
    {reconciliation !== undefined && <div role="status" className="space-y-2"><p>Authoritative GET completed. Review the current document state before deciding whether to submit another mutation.</p><CanonicalInspector label="Reconciliation GET" text={reconciliation?.canonical ?? 'GET returned canonical document absence.'}/></div>}
    <CanonicalInspector label="Mutation request" text={prepared.value?.canonical ?? 'Correct malformed input to inspect the request.'}/>
    <button className="primary-button" onClick={() => { void submit() }} disabled={running || reconciling || !!unknownMutation || !prepared.value || (state.operation === 'delete' && !confirmed)}>{running ? `Running ${state.operation}…` : basic ? 'Save document' : 'Submit document mutation'}</button>
    {running && <p role="status">Mutation is running; after acknowledgement, fresh backend GET/search reads will run.</p>}
    {result && <div role="status" className="space-y-2"><p>{result.writeOutcome === 'not_deleted' ? 'Delete acknowledged; the backend reports that no document was deleted. Fresh backend GET/search completed and GET confirms absence.' : basic ? 'Document saved. Search Kernel returned the saved document. Try your first search above.' : `${result.acknowledgement.operation} acknowledged. Fresh backend GET/search completed; GET confirms document ${result.readbackPresence}.`}</p><CanonicalInspector label="Mutation response" text={result.acknowledgement.canonicalResponse}/><CanonicalInspector label="Read-back GET" text={result.document?.canonical ?? 'GET returned canonical document absence.'}/><CanonicalInspector label="Read-back search" text={result.search.canonicalResponse ?? 'Canonical response unavailable'}/>{result.document && <button className="soft-button" onClick={() => open(result.document!.id)}>Open read-back document</button>}</div>}
  </section>
}
