import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router-dom'
import { useAdminClient, useCapabilities, useSearchKernelClient } from './client-context'
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
  const client = useSearchKernelClient()
  const adminClient = useAdminClient()
  const capabilities = useCapabilities()
  const workspace = tabFor(page)
  const navigation = navigationFor(capabilities.capabilities)
  const { data: index, isLoading, isFetching, error, refetch } = useQuery({ queryKey: ['index', indexId], queryFn: clientQuery(signal => client.getIndex(indexId!, { signal })), ...clientQueryDefaults, staleTime: 0, refetchOnMount: 'always', refetchOnWindowFocus: false, enabled: Boolean(workspace && indexId) })
  const { data: summary, error: summaryError, refetch: refetchSummary } = useQuery({ queryKey: ['index-summary', indexId], queryFn: clientQuery(async () => (await adminClient.getIndexSummary(indexId!)) ?? null), ...clientQueryDefaults, enabled: Boolean(workspace && indexId && capabilities.capabilities?.admin.overview) })
  const navigate: Nav = (next, targetIndexId = indexId) => { routerNavigate(pagePath(next, targetIndexId)); setMobileNav(false) }
  useEffect(() => {
    const shortcut = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setCommandOpen(true) } }
    addEventListener('keydown', shortcut)
    return () => removeEventListener('keydown', shortcut)
  }, [])
  if (workspace && (isLoading || isFetching)) return <Loading />
  if (workspace && error) return <div className="mx-auto max-w-xl p-6"><QueryErrorPanel error={error} retry={() => { void refetch() }} /></div>
  if (workspace && !index) return <NotFound/>
  const currentNavigation = workspace && (navigation.tabs.find(item => item.page === page) ?? navigation.sections[workspace].find(item => item.page === page))
  if (workspace && capabilities.capabilities && currentNavigation && !currentNavigation.enabled) return <UnavailableFeature title={currentNavigation.label} />
  return <div className="min-h-screen bg-slate-50">
    <GlobalSidebar page={page} navigate={navigate} open={mobileNav} onClose={() => setMobileNav(false)} />
    <main className="min-h-screen lg:pl-64">
      <div className="lg:hidden flex h-14 items-center justify-between border-b border-slate-200 bg-white px-4"><Brand/><MobileMenuButton open={() => setMobileNav(true)} /></div>
      {workspace && <IndexHeader current={workspace} index={index!} summary={summary ?? undefined} navigate={navigate} />}
      <div className="mx-auto max-w-[1680px] px-4 py-6 sm:px-6 lg:px-8">
        {Boolean(error) && index && <QueryErrorBanner error={error} retry={() => { void refetch() }} />}
        {Boolean(summaryError) && <QueryErrorBanner error={summaryError} retry={() => { void refetchSummary() }} />}
        {capabilities.error && <QueryErrorBanner error={capabilities.error} retry={() => { void capabilities.retry() }} />}
        <IndexContext.Provider value={index ?? null}>{workspace && navigation.sections[workspace].length > 0 ? <div className="grid gap-6 lg:grid-cols-[180px_minmax(0,1fr)]"><SectionNavigation tab={workspace} page={page} navigate={navigate}/><Page page={page} navigate={navigate}/></div> : <Page page={page} navigate={navigate}/>}</IndexContext.Provider>
      </div>
    </main>
    {commandOpen && <CommandPalette close={() => setCommandOpen(false)} navigate={navigate} indexId={indexId} />}
  </div>
}
