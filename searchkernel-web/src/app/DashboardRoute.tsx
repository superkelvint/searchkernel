import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useAdminClient, useCapabilities, useSearchKernelClient } from './client-context'
import { IndexTargetT, targetKey } from '../api/searchkernel-client'
import { IndexContext } from './index-context'
import { Loading, NotFound, UnavailableFeature } from './fallback-pages'
import { Page } from './Page'
import { navigationFor, pagePath, tabFor, type Nav, type Page as PageName } from './navigation'
import { Brand, GlobalSidebar, IndexHeader, MobileMenuButton, SectionNavigation } from '../components/DashboardChrome'
import { CommandPalette } from '../components/CommandPalette'
import { clientQuery, clientQueryDefaults, queryError } from './query'
import { QueryErrorBanner, QueryErrorPanel } from '../components/Shared'

export function DashboardRoute({page}:{page: PageName}) {
  const [commandOpen, setCommandOpen] = useState(false)
  const [mobileNav, setMobileNav] = useState(false)
  const routerNavigate = useNavigate()
  const { indexId } = useParams()
  const [params, setParams] = useSearchParams()
  const generation = params.get('generation')
  const target = generation && indexId ? new IndexTargetT(indexId, generation) : indexId!
  const client = useSearchKernelClient()
  const adminClient = useAdminClient()
  const capabilities = useCapabilities()
  const workspace = tabFor(page)
  const navigation = navigationFor(capabilities.capabilities)
  const { data: index, isLoading, error, refetch } = useQuery({ queryKey: ['index', indexId, generation], queryFn: clientQuery(signal => client.getIndex(target, { signal })), ...clientQueryDefaults, refetchOnWindowFocus: false, enabled: Boolean(workspace && indexId) })
  const registry = useQuery({ queryKey: ['daemon-status'], queryFn: clientQuery(signal => client.daemonStatus!({ signal })), ...clientQueryDefaults, enabled: !!client.daemonStatus && !!workspace })
  const { data: summary, error: summaryError, refetch: refetchSummary } = useQuery({ queryKey: ['index-summary', indexId, generation], queryFn: clientQuery(async () => (await adminClient.getIndexSummary(indexId!)) ?? null), ...clientQueryDefaults, enabled: Boolean(workspace && indexId && capabilities.capabilities?.admin.overview) })
  const navigate: Nav = (next, targetIndexId = indexId) => { routerNavigate(pagePath(next, targetIndexId) + (targetIndexId === indexId && generation ? `?generation=${encodeURIComponent(generation)}` : '')); setMobileNav(false) }
  useEffect(() => {
    if (index?.target && !generation) {
      setParams(previous => { const next = new URLSearchParams(previous); next.set('generation', String(index.target!.generation)); return next }, { replace: true })
    }
  }, [index, generation, setParams])
  useEffect(() => {
    const shortcut = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setCommandOpen(true) } }
    addEventListener('keydown', shortcut)
    return () => removeEventListener('keydown', shortcut)
  }, [])
  if (workspace && isLoading) return <Loading />
  if (workspace && error) return <div className="mx-auto max-w-xl p-6"><p className="mb-3">Selected target: <code>{indexId}</code> · generation <code>{generation ?? 'Not established'}</code></p><QueryErrorPanel error={error} retry={() => { void refetch() }} /><button className="primary-button" onClick={() => routerNavigate('/indexes')}>Refresh inventory / select current incarnation</button></div>
  if (workspace && !index) return <NotFound/>
  const currentNavigation = workspace && (navigation.tabs.find(item => item.page === page) ?? navigation.sections[workspace].find(item => item.page === page))
  if (workspace && capabilities.capabilities && currentNavigation && !currentNavigation.enabled) return <UnavailableFeature title={currentNavigation.label} />
  return <div className="min-h-screen bg-slate-50">
    <GlobalSidebar page={page} navigate={navigate} open={mobileNav} onClose={() => setMobileNav(false)} />
    <main className="min-h-screen lg:pl-64">
      <div className="lg:hidden flex h-14 items-center justify-between border-b border-slate-200 bg-white px-4"><Brand/><MobileMenuButton open={() => setMobileNav(true)} /></div>
      {workspace && <><IndexHeader current={workspace} index={index!} summary={summary ?? undefined} navigate={navigate} /><details className="px-8 py-2 text-xs"><summary className="cursor-pointer">Index connection details</summary>Target: <code>{index!.id}</code> · generation <code>{index!.generation ?? 'Unavailable'}</code> <button className="soft-button" onClick={() => { void refetch(); void registry.refetch() }}>Refresh selected incarnation</button></details></>}
      <div className="mx-auto max-w-[1680px] px-4 py-6 sm:px-6 lg:px-8">
        {Boolean(error) && index && <QueryErrorBanner error={error} retry={() => { void refetch() }} />}
        {Boolean(summaryError) && <QueryErrorBanner error={summaryError} retry={() => { void refetchSummary() }} />}
        {capabilities.error && <QueryErrorBanner error={capabilities.error} retry={() => { void capabilities.retry() }} />}
        <IndexContext.Provider value={index ?? null} key={index?.target ? targetKey(index.target) : indexId}>{workspace && navigation.sections[workspace].length > 0 ? <div className="grid gap-6 lg:grid-cols-[180px_minmax(0,1fr)]"><SectionNavigation tab={workspace} page={page} navigate={navigate}/><Page page={page} navigate={navigate}/></div> : <Page page={page} navigate={navigate}/>}</IndexContext.Provider>
      </div>
    </main>
    {commandOpen && <CommandPalette close={() => setCommandOpen(false)} navigate={navigate} indexId={indexId} />}
  </div>
}
