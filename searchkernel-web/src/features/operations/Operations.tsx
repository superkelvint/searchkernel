import { useQuery } from '@tanstack/react-query'
import { useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { clientQuery, clientQueryDefaults } from '../../app/query'
import { Heading } from '../../components/Shared'
import { CanonicalInspector, ErrorNotice } from '../../components/Workbench'

export function GlobalOperations() {
  const client = useSearchKernelClient()
  const status = useQuery({ queryKey: ['daemon-status'], queryFn: clientQuery(signal => client.daemonStatus!({ signal })), ...clientQueryDefaults, enabled: !!client.daemonStatus })
  return <><Heading title="Daemon health and runtime diagnostics" description="Read-only status from the daemon registry."><button className="soft-button" disabled={status.isFetching} onClick={() => { void status.refetch() }}>Refresh health</button></Heading><ErrorNotice error={status.error}/>{status.data && !status.error ? <section className="surface space-y-3 p-5"><p>{status.data.ready ? 'Ready' : 'Not ready'}</p><p>Version: {status.data.version}</p><p>Native runtime: {status.data.native_runtime}</p><p>Lifecycle: {status.data.lifecycle}</p><p>Open indexes: {status.data.indexes.filter(item => item.state === 'open').length}</p><p>Last fetched: {new Date(status.dataUpdatedAt).toLocaleString()}</p><p>Unavailable: {status.data.unavailable.join(', ')}</p><CanonicalInspector label="Runtime diagnostics" text={JSON.stringify(status.data, null, 2)}/></section> : <p>{status.error ? 'Daemon unavailable. Retry or refresh health.' : client.daemonStatus ? 'Fetching daemon status…' : 'Diagnostics require daemon mode.'}</p>}</>
}

function IndexOperationalStatus({ raw = false }: { raw?: boolean }) {
  const index = useIndex(), client = useSearchKernelClient()
  const diagnostics = useQuery({ queryKey: ['index-diagnostics', index.id, index.generation], queryFn: clientQuery(signal => client.indexDiagnostics!(index.target!, { signal })), ...clientQueryDefaults, enabled: !!index.target && !!client.indexDiagnostics })
  return <><Heading title={raw ? 'Index diagnostics' : 'Index health'} description="Canonical target and authoritative registry status."><button className="soft-button" disabled={diagnostics.isFetching} onClick={() => { void diagnostics.refetch() }}>Refresh index status</button></Heading><ErrorNotice error={diagnostics.error}/><section className="surface space-y-3 p-5"><p>Logical name: <code>{index.id}</code></p><p>Generation: <code>{index.generation ?? 'Unavailable'}</code></p>{diagnostics.data && !diagnostics.error ? <><p>Lifecycle: {diagnostics.data.status.state}</p><p>Persisted: {diagnostics.data.status.persisted == null ? 'Unknown' : String(diagnostics.data.status.persisted)}</p><p>Schema identity: {diagnostics.data.status.schema_name ?? 'Unavailable'}</p><p>{diagnostics.data.status.warning}</p><p>Last fetched: {new Date(diagnostics.dataUpdatedAt).toLocaleString()}</p><p>Counts, resource metrics, publication status, and lifecycle history are unavailable.</p>{raw && <><CanonicalInspector label="Canonical target" text={JSON.stringify({ name: index.id, generation: index.generation }, null, 2)}/><CanonicalInspector label="Index diagnostics" text={diagnostics.data.canonical}/></>}</> : <p>{diagnostics.error ? 'Selected incarnation could not be verified. Refresh inventory and explicitly select the current incarnation.' : diagnostics.isFetching ? 'Fetching index status…' : 'Production index diagnostics require daemon mode.'}</p>}</section></>
}
export function Health() { return <IndexOperationalStatus/> }
export function Diagnostics() { return <IndexOperationalStatus raw/> }
