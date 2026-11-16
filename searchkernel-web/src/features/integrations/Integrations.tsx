import { useMemo, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { targetKey } from '../../api/searchkernel-client'
import { useSearchKernelClient } from '../../app/client-context'
import { useIndex } from '../../app/index-context'
import { clientQuery } from '../../app/query'
import { Heading, QueryErrorPanel } from '../../components/Shared'
import { CanonicalInspector, CopyButton } from '../../components/Workbench'
import {
  instantSearchAdapterVersion,
  instantSearchApplicationExamples,
  instantSearchCompatibility,
  instantSearchInstallCommands,
  runInstantSearchPreview,
  validateIntegrationIndex,
} from './integration-service'

function SetupCard({ title, install, code }: { title: string; install: string; code?: string }) {
  return <div className="rounded-lg border border-slate-200 p-4 space-y-3">
    <h3 className="font-medium">{title}</h3>
    <div>
      <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Install</p>
      <pre className="mt-1 overflow-auto rounded bg-slate-950 p-3 font-mono text-xs text-slate-100">{install}</pre>
      <CopyButton text={install} label={`Copy ${title} install command`} />
    </div>
    {code ? <div>
      <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Application code</p>
      <pre className="mt-1 max-h-96 overflow-auto rounded bg-slate-950 p-3 font-mono text-xs text-slate-100">{code}</pre>
      <CopyButton text={code} label={`Copy ${title} example`} />
    </div> : <p className="text-sm text-slate-600">Enter a valid public Search Endpoint URL and alias above to generate runnable code without placeholder endpoint values.</p>}
  </div>
}

export function Integrations() {
  const selected = useIndex()
  const client = useSearchKernelClient()
  const [publicEndpoint, setPublicEndpoint] = useState('')
  const [publicAlias, setPublicAlias] = useState('')
  const [previewQuery, setPreviewQuery] = useState('')
  const identity = useQuery({
    queryKey: ['integration-index', selected.target ? targetKey(selected.target) : selected.id],
    queryFn: clientQuery(signal => validateIntegrationIndex(client, selected, { signal })),
    retry: false,
    staleTime: 0,
    refetchInterval: 10_000,
    refetchOnWindowFocus: 'always',
  })
  const verified = !identity.isFetching && !identity.error && !!identity.data
  const preview = useMutation({ mutationFn: () => runInstantSearchPreview(publicEndpoint, publicAlias, previewQuery) })
  const examples = useMemo(() => {
    try { return instantSearchApplicationExamples(publicEndpoint, publicAlias) } catch { return undefined }
  }, [publicEndpoint, publicAlias])
  const widgetEntries = Object.entries(instantSearchCompatibility.widgets)
  const supportedWidgets = widgetEntries.filter(([, value]) => value.status !== 'unsupported')
  const resetPreview = () => preview.reset()

  return <div className="space-y-5">
    <Heading title="InstantSearch.js" description={`Application integration for logical index ${selected.id}`} />

    <section className="surface p-5 space-y-3" aria-label="Integration status">
      <h2 className="text-base font-semibold">Selected index identity</h2>
      <p role="status">{identity.isFetching ? 'Checking current index identity…' : identity.error ? 'Index validation failed. Integration launch is unavailable.' : 'Selected generation is current. Public endpoint binding is still a separate deployment concern.'}</p>
      {identity.error && <QueryErrorPanel error={identity.error} retry={() => { void identity.refetch() }} />}
      {verified && <p className="text-sm text-slate-600">The selected generation responded through the trusted index client. This validates the admin workspace only; it does not prove which public alias targets this index.</p>}
      <button className="soft-button" disabled={identity.isFetching} onClick={() => { void identity.refetch() }}>Recheck index identity</button>
      <dl className="text-sm"><dt className="font-medium">Logical index</dt><dd className="font-mono break-all">{selected.id}</dd><dt className="mt-2 font-medium">Selected process generation</dt><dd className="font-mono break-all">{selected.generation ?? 'Unavailable'}</dd></dl>
      <CopyButton text={selected.id} label="Copy logical index" />
    </section>

    <section className="surface p-5 space-y-4" aria-label="Public endpoint">
      <div>
        <h2 className="text-base font-semibold">Public Search Endpoint</h2>
        <p className="mt-1 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm">Browser application → bounded public Search Endpoint. Never point application code or this preview at the mutation-capable trusted administration listener.</p>
      </div>
      <p className="text-sm text-slate-600">The public Search Endpoint implementation has landed. Dashboard discovery/create/update APIs have not, so W6 cannot authoritatively select an endpoint from daemon state yet. You can supply an already configured endpoint to exercise the real adapter. These values stay in this page only and are not treated as canonical configuration.</p>
      <div className="grid gap-3 md:grid-cols-2">
        <label className="text-sm font-medium">Public Search Endpoint URL
          <input className="mt-1 w-full rounded border border-slate-300 px-3 py-2 font-mono text-sm" aria-label="Public Search Endpoint URL" placeholder="http://127.0.0.1:9001/search" value={publicEndpoint} onChange={event => { setPublicEndpoint(event.target.value); resetPreview() }} />
        </label>
        <label className="text-sm font-medium">Public endpoint alias
          <input className="mt-1 w-full rounded border border-slate-300 px-3 py-2 font-mono text-sm" aria-label="Public endpoint alias" placeholder="products" value={publicAlias} onChange={event => { setPublicAlias(event.target.value); resetPreview() }} />
        </label>
      </div>
      <p className="text-xs text-slate-500">The alias is the Search Endpoint configuration key and may differ from logical index <code>{selected.id}</code>. A successful preview proves that alias is serving; it does not prove its generation binding matches this selected admin target.</p>
      <p className="text-sm">Endpoint exposure, fields/facets/highlights/sorts, listener binding, allowed origins and durable restart-safe target binding remain owned by <code>searchkerneld</code>.</p>
    </section>

    <section className="surface p-5 space-y-3" aria-label="Compatibility">
      <h2 className="text-base font-semibold">Authoritative compatibility</h2>
      <p className="text-sm">Profile <code>{instantSearchCompatibility.profile}</code> is <strong>{instantSearchCompatibility.releaseStatus}</strong> for <code>@searchkernel/instantsearch@{instantSearchAdapterVersion}</code>. This page consumes the declaration from <code>integrations/instantsearch/src/compatibility.json</code>; it does not maintain a second widget matrix.</p>
      <p className="text-sm text-slate-600">{supportedWidgets.length} of {widgetEntries.length} declared InstantSearch.js widget surfaces are supported with documented constraints. React and Vue consume the same adapter and underlying capability profile.</p>
      <div className="flex flex-wrap gap-2" aria-label="Supported InstantSearch widgets">
        {supportedWidgets.map(([name, value]) => <span key={name} title={'constraints' in value ? value.constraints : ''} className="rounded-full border border-slate-300 bg-white px-2 py-1 text-xs">{name}</span>)}
      </div>
      {Object.entries(instantSearchCompatibility.frameworks).map(([name, framework]) => <div key={name} className="rounded border border-slate-200 p-3">
        <p className="font-medium">{name} · {framework.status}</p>
        <p className="mt-1 text-sm text-slate-600">{framework.constraints}</p>
      </div>)}
      <div className="flex flex-wrap gap-2"><CopyButton text="./dev test instantsearch-compat" label="Copy compatibility verification command" /></div>
      <CanonicalInspector label="Authoritative compatibility declaration" text={JSON.stringify(instantSearchCompatibility, null, 2)} />
    </section>

    <section className="surface p-5 space-y-3" aria-label="Live preview">
      <h2 className="text-base font-semibold">Live adapter preview</h2>
      <p className="text-sm text-slate-600">This executes the landed <code>createSearchKernelInstantSearchClient</code> against the public endpoint you supplied. It uses the same translation/projection boundary as an application; trusted Search Lab results are not substituted.</p>
      <label className="block text-sm font-medium">Preview query
        <input className="mt-1 w-full rounded border border-slate-300 px-3 py-2" aria-label="Preview query" value={previewQuery} onChange={event => { setPreviewQuery(event.target.value); resetPreview() }} />
      </label>
      <button className="primary-button" disabled={!verified || !publicEndpoint.trim() || !publicAlias.trim() || preview.isPending} onClick={() => preview.mutate()}>{preview.isPending ? 'Running preview…' : 'Run live adapter preview'}</button>
      {!verified && <p className="text-xs text-slate-500">Preview stays disabled until the selected admin target is verified current.</p>}
      {preview.error && <QueryErrorPanel error={preview.error} retry={() => preview.mutate()} />}
      {preview.data && <><p role="status" className="text-sm">Public adapter preview succeeded · {String(preview.data.nbHits ?? 'unknown')} matches.</p><CanonicalInspector label="InstantSearch preview result" text={JSON.stringify(preview.data, null, 2)} /></>}
    </section>

    <section className="surface p-5 space-y-4" aria-label="Application setup">
      <h2 className="text-base font-semibold">Application setup</h2>
      <p className="text-sm text-slate-600">Install commands are generated from the pinned compatibility declaration and adapter package version. Once the public URL and alias are valid, examples are populated from those explicit values rather than generic endpoint placeholders.</p>
      <SetupCard title="Vanilla InstantSearch.js" install={instantSearchInstallCommands.vanilla} code={examples?.vanilla} />
      <SetupCard title="React InstantSearch" install={instantSearchInstallCommands.react} code={examples?.react} />
      <SetupCard title="Vue InstantSearch" install={instantSearchInstallCommands.vue} code={examples?.vue} />
    </section>
  </div>
}
