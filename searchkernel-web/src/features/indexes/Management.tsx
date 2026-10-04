import { useEffect, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { IndexTargetT, type IndexStatus } from '../../api/searchkernel-client'
import { useCapabilities, useSearchKernelClient } from '../../app/client-context'
import { clientQuery, clientQueryDefaults } from '../../app/query'
import { Heading } from '../../components/Shared'
import { CanonicalInspector, ErrorNotice } from '../../components/Workbench'
import { canDelete } from './lifecycle'

export function IndexManagement() {
  const client = useSearchKernelClient(), cache = useQueryClient()
  const selectionSequence = useRef(0)
  const navigate = useNavigate()
  const inventory = useQuery({ queryKey: ['indexes'], queryFn: clientQuery(signal => client.listIndexes({ signal })), ...clientQueryDefaults })
  const status = useQuery({ queryKey: ['daemon-status'], queryFn: clientQuery(signal => client.daemonStatus!({ signal })), ...clientQueryDefaults, enabled: !!client.daemonStatus })
  const { capabilities } = useCapabilities()
  async function refresh() {
    await Promise.all([cache.invalidateQueries({ queryKey: ['indexes'] }), cache.invalidateQueries({ queryKey: ['daemon-status'] })])
  }
  const names = new Set(inventory.data?.map(item => item.id) ?? [])
  status.data?.indexes.forEach(item => names.add(item.name))
  return <><Heading title="Search indexes" description="Durable discovery and authoritative daemon lifecycle state."><button className="soft-button" onClick={() => { void refresh() }}>Refresh inventory and health</button><button className="primary-button" disabled={!capabilities?.indexes.create} onClick={() => navigate('/indexes/new')}>Create index</button></Heading>
    <section className="surface mb-5 p-5"><h2 className="font-semibold">Daemon health</h2><ErrorNotice error={status.error}/>{status.data && !status.error ? <><p>{status.data.ready ? 'Ready' : 'Not ready'} · version {status.data.version} · native runtime {status.data.native_runtime} · lifecycle {status.data.lifecycle}</p><p className="text-xs">Fetched {new Date(status.dataUpdatedAt).toLocaleString()}</p><CanonicalInspector label="Runtime diagnostics" text={JSON.stringify(status.data, null, 2)}/><p>Unavailable: {status.data.unavailable.join(', ')}</p></> : <p>{client.daemonStatus ? status.error ? 'Daemon unavailable' : 'Fetching health…' : 'Production diagnostics require daemon mode.'}</p>}</section>
    <ErrorNotice error={inventory.error}/>
    <section className="surface p-5"><h2 className="font-semibold">Index inventory</h2>{inventory.isPending && <p>Discovering indexes…</p>}{!inventory.isPending && !inventory.error && names.size === 0 && <p>No persisted indexes.</p>}
      {[...names].sort().map(name => <IndexRow key={name} name={name} status={status.data?.indexes.find(item => item.name === name)} refresh={refresh} beginSelection={() => ++selectionSequence.current} select={(generation, sequence) => { if (sequence === selectionSequence.current) navigate(`/indexes/${encodeURIComponent(name)}/overview?generation=${encodeURIComponent(generation)}`) }}/>)}</section>

  </>
}

function IndexRow({ name, status, refresh, beginSelection, select }: { name: string; status?: IndexStatus; refresh: () => Promise<void>; beginSelection: () => number; select: (generation: string, sequence: number) => void }) {
  const client = useSearchKernelClient(), cache = useQueryClient(), navigate = useNavigate()
  const [operation, setOperation] = useState('idle'), [error, setError] = useState<unknown>()
  const [confirmation, setConfirmation] = useState(''), [deleting, setDeleting] = useState<IndexTargetT>()
  const running = useRef(false), mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  async function open() {
    if (running.current) return
    const sequence = beginSelection()
    running.current = true; setOperation('opening'); setError(undefined)
    try {
      const index = await client.openIndex!(name)
      await refresh()
      setOperation('opened')
      if (mounted.current) select(index.generation!, sequence)
    } catch (error) { setError(error); setOperation('error') }
    finally { running.current = false }
  }
  async function remove() {
    if (!deleting || running.current || !canDelete(deleting, confirmation, false)) return
    const target = deleting
    running.current = true; setOperation('deleting'); setError(undefined)
    try {
      await client.deleteIndex!(target)
      // Invalidate ONLY this exact incarnation, including in-flight reads.
      const matches = (query: { queryKey: readonly unknown[] }) => query.queryKey[1] === target.name && query.queryKey[2] === target.generation
      await cache.cancelQueries({ predicate: matches })
      cache.removeQueries({ predicate: matches })
      setDeleting(undefined); setConfirmation(''); setOperation('deleted')
      await refresh()
    } catch (error) { setError(error); setOperation('error'); await refresh() }
    finally { running.current = false }
  }
  return <article className="space-y-2 border-t py-4" aria-label={`Index ${name}`}><div className="flex flex-wrap items-center gap-3"><button className="font-mono text-indigo-700" disabled={operation === 'opening' || operation === 'deleting'} onClick={() => { if (client.openIndex) void open(); else navigate(`/indexes/${name}/overview`) }}>{name}</button><span>{status?.state ?? 'Persisted target; runtime state unavailable'}</span><button className="soft-button" disabled={!client.openIndex || operation === 'opening' || operation === 'deleting'} onClick={() => { void open() }}>{status?.state === 'open' ? 'Select / open' : 'Open index'}</button>{status?.generation && client.deleteIndex && <button className="soft-button text-red-700" disabled={operation === 'opening' || operation === 'deleting'} onClick={() => { setDeleting(new IndexTargetT(name, status.generation)); setConfirmation('') }}>Delete index</button>}</div><p className="text-xs">Generation: <code>{status?.generation ?? 'None; open to obtain an incarnation'}</code> · Persisted: {status?.persisted == null ? 'Unknown' : String(status.persisted)}</p>{status?.warning && <p>{status.warning}</p>}<p role="status">{operation}</p><ErrorNotice error={error}/>
    {deleting && <div role="dialog" aria-label="Confirm index deletion" className="rounded border border-red-300 p-4"><h3>Delete index {String(deleting.name)}</h3><p>This permanently removes the index and its persisted data. Logical name: <code>{String(deleting.name)}</code>. Exact incarnation: <code>{String(deleting.generation)}</code>.</p><label>Type the index name<input aria-label="Confirm index name" className="input" value={confirmation} onChange={event => setConfirmation(event.target.value)}/></label><button className="primary-button" disabled={!canDelete(deleting, confirmation, operation === 'deleting')} onClick={() => { void remove() }}>Permanently delete index</button> <button className="soft-button" disabled={operation === 'deleting'} onClick={() => setDeleting(undefined)}>Cancel</button></div>}
  </article>
}
