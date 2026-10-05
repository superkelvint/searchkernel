import { CollectionNextStep } from '../onboarding/FirstSearch'
import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Dialog, DialogBackdrop, DialogPanel, DialogTitle } from '@headlessui/react'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useCapabilities, useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { clientQuery, clientQueryDefaults } from '../../app/query'
import { Heading } from '../../components/Shared'
import { CanonicalInspector, CopyButton, ErrorNotice } from '../../components/Workbench'
import { decodeSearchParams, encodeSearchParams, updateSearchState, canAdvancePage } from '../search-lab/search-params'
import { DocumentEditor } from './DocumentEditor'
import { decodeDocumentId, documentPath } from './workflow'

export function Documents() {
  const index = useIndex(), client = useSearchKernelClient(), capabilities = useCapabilities()
  const { documentId: legacyId, documentToken } = useParams()
  const [params, setParams] = useSearchParams(), navigate = useNavigate()
  const [creating, setCreating] = useState(false)
  const restored = useMemo<ReturnType<typeof decodeSearchParams>>(() => params.has('s') ? decodeSearchParams(params) : { state: { query: params.get('query') ?? '', limit: 20 } }, [params])
  const state = restored.state
  const prepared = useMemo(() => { try { return { value: client.prepareSearch(state) } } catch (error) { return { error } } }, [client, state])
  const selected = useMemo(() => { try { return { id: documentToken ? decodeDocumentId(documentToken) : legacyId } } catch (error) { return { error } } }, [documentToken, legacyId])
  const search = useQuery({ queryKey: ['document-browse', index.id, index.generation, prepared.value?.canonical], queryFn: clientQuery(signal => client.executeSearch(index.target ?? index.id, prepared.value!, { signal })), ...clientQueryDefaults, enabled: !!prepared.value && !restored.error })
  const document = useQuery({ queryKey: ['document', index.id, index.generation, selected.id], queryFn: clientQuery(async signal => (await client.getDocument(index.target ?? index.id, selected.id!, { signal })) ?? null), ...clientQueryDefaults, enabled: selected.id !== undefined })
  const rows = search.data?.hits ?? []
  const update = (patch: Parameters<typeof updateSearchState>[1]) => setParams(() => { const next = encodeSearchParams(updateSearchState(state, patch)); if (index.generation) next.set('generation', index.generation); if (guided) next.set('journey', 'first-search'); return next })
  const pageParams = (offset: number) => { const next = encodeSearchParams({ ...state, offset }); if (index.generation) next.set('generation', index.generation); if (guided) next.set('journey', 'first-search'); return next }
  const open = (id: string) => { setCreating(false); navigate(`${documentPath(index.id, id)}?${params}`) }
  const close = () => navigate(`/indexes/${encodeURIComponent(index.id)}/data/documents?${params}`)
  const guided = params.get('journey') === 'first-search'
  const writable = capabilities.capabilities?.documents.write === true
  const editorKey = JSON.stringify([index.id, index.generation, selected.id])
  return <div className="min-w-0">
    <Heading title="Documents" description={`Backend document reads and mutations in ${index.id}.`}><CopyButton label="Copy document URL" text={location.href}/>{writable && <button className="primary-button ml-2" onClick={() => setCreating(value => !value)}>{guided ? "Add first document" : "Create document"}</button>}</Heading>
    <CollectionNextStep page="documents"/>
    <ErrorNotice error={restored.error ? new Error(restored.error) : prepared.error ?? selected.error ?? search.error}/>
    <section className="surface space-y-3 p-4">
      <label className="block">Search documents<input aria-label="Search documents" className="input" value={state.query} onChange={e => update({ query: e.target.value, mode: 'text' })} placeholder="Empty query browses the index"/></label>
      <div className="flex flex-wrap items-end gap-3"><label>Rows per page<input aria-label="Document page size" className="input w-24" type="number" value={state.limit ?? ''} onChange={e => update({ limit: e.target.value === '' ? undefined : Number(e.target.value) })}/></label><button className="soft-button" onClick={() => navigate(`/indexes/${encodeURIComponent(index.id)}/search/lab?${pageParams(state.offset ?? 0)}`)}>Open in Search Lab</button><button className="soft-button" onClick={() => { void search.refetch() }}>Refresh documents</button></div>
      {search.isFetching && <p role="status">Fetching documents…</p>}
      {search.data && <p>{search.data.matches} matching documents · {search.data.totalRelation} · offset {state.offset ?? 0}</p>}
      {!search.isFetching && search.data && !rows.length && <p>{String(search.data.matches) === '0' ? 'No matching documents. Create a document or change the search.' : 'No documents on this page.'}</p>}
      <div className="space-y-3">{rows.map(hit => <article key={hit.document.id} className="rounded border border-slate-200 p-3"><div className="flex flex-wrap justify-between gap-2"><code>{hit.document.id}</code><button className="soft-button" aria-label={`Open ${hit.document.id}`} onClick={() => open(hit.document.id)}>Open document</button></div><pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap text-xs">{JSON.stringify(hit.document.fields, null, 2)}</pre></article>)}</div>
      <div className="flex flex-wrap gap-2"><button className="soft-button" disabled={search.isFetching || search.isError || !state.limit || !state.offset} onClick={() => setParams(pageParams(Math.max(0, (state.offset ?? 0) - state.limit!)))}>Previous documents</button><button className="soft-button" disabled={search.isFetching || search.isError || !canAdvancePage(state, search.data ?? null)} onClick={() => setParams(pageParams((state.offset ?? 0) + state.limit!))}>Next documents</button></div>
      <CanonicalInspector label="Browse request" text={prepared.value?.canonical ?? 'Invalid request'}/>{search.data?.canonicalResponse && <CanonicalInspector label="Browse response" text={search.data.canonicalResponse}/>}
    </section>
    {creating && writable && prepared.value && <DocumentEditor key={JSON.stringify([index.id, index.generation])} browse={prepared.value} open={open} guided={guided}/>}
    {(selected.id !== undefined || !!selected.error) && <Dialog open onClose={close} className="relative z-50">
      <DialogBackdrop className="fixed inset-0 bg-slate-950/25"/>
      <div className="fixed inset-0 overflow-y-auto"><DialogPanel className="ml-auto min-h-screen w-full max-w-3xl bg-white p-5 shadow-xl">
        <div className="flex justify-between"><DialogTitle className="font-mono">{selected.id ?? 'Invalid document URL'}</DialogTitle><button aria-label="Close document" className="soft-button" onClick={close}>Close</button></div>
        <div className="mt-2 flex flex-wrap gap-2"><CopyButton label="Copy detail URL" text={location.href}/><button className="soft-button" disabled={selected.id === undefined || document.isFetching} onClick={() => { void document.refetch() }}>Refresh document</button></div>
        {document.isFetching && <p role="status">Reading document from Search Kernel…</p>}
        <ErrorNotice error={selected.error ?? document.error}/>
        {!document.isFetching && !document.error && !document.data && <p className="mt-4">The requested document was not found.</p>}
        {document.data && <><dl className="mt-4 space-y-3">{Object.entries(document.data.fields).map(([name, value]) => <div key={name}><dt className="font-mono text-xs text-slate-500">{name}</dt><dd>{typeof value === 'string' ? value : <pre className="overflow-auto whitespace-pre-wrap text-xs">{JSON.stringify(value, null, 2)}</pre>}</dd></div>)}</dl>{document.data.canonical && <CanonicalInspector label="Canonical GET" text={document.data.canonical}/>}</>}
        {writable && prepared.value && selected.id !== undefined && !document.isLoading && <DocumentEditor key={editorKey} document={document.data ?? undefined} id={selected.id} browse={prepared.value} open={open}/>}
      </DialogPanel></div>
    </Dialog>}
  </div>
}
