import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router-dom'
import { InformationCircleIcon } from '@heroicons/react/24/outline'
import { useCapabilities, useSearchKernelClient } from './client-context'
import { IndexContext } from './index-context'
import { Loading, NotFound, QueryErrorState, UnavailableFeature } from './fallback-pages'
import { Page } from './Page'
import { navigationFor, pagePath, tabFor, type Nav, type Page as PageName } from './navigation'
import { Brand, GlobalSidebar, IndexHeader, MobileMenuButton, SectionNavigation } from '../components/DashboardChrome'
import { CommandPalette } from '../components/CommandPalette'
import { clientQuery, clientQueryDefaults, queryError } from './query'
import { QueryErrorBanner } from '../components/Shared'

export function DashboardRoute({page}:{page: PageName}) {
  const [commandOpen, setCommandOpen] = useState(false)
  const [mobileNav, setMobileNav] = useState(false)
  const routerNavigate = useNavigate()
  const { indexId = 'products' } = useParams()
  const client = useSearchKernelClient()
  const capabilities = useCapabilities()
  const workspace = tabFor(page)
  const navigation = navigationFor(capabilities.capabilities)
  const { data: index, isLoading, error, refetch } = useQuery({ queryKey: ['index', indexId], queryFn: clientQuery(() => client.getIndex(indexId)), ...clientQueryDefaults, enabled: Boolean(workspace) })
  const navigate: Nav = (next, targetIndexId = indexId) => { routerNavigate(pagePath(next, targetIndexId)); setMobileNav(false) }
  useEffect(() => {
    const shortcut = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setCommandOpen(true) } }
    addEventListener('keydown', shortcut)
    return () => removeEventListener('keydown', shortcut)
  }, [])
  if (workspace && isLoading) return <Loading />
  if (workspace && error && !index) return <QueryErrorState message={queryError(error)?.message ?? 'The index could not be loaded.'} retry={() => { void refetch() }} />
  if (workspace && !index) return <NotFound/>
  const currentNavigation = workspace && (navigation.tabs.find(item => item.page === page) ?? navigation.sections[workspace].find(item => item.page === page))
  if (workspace && capabilities.capabilities && currentNavigation && !currentNavigation.enabled) return <UnavailableFeature title={currentNavigation.label} />
  return <div className="min-h-screen bg-slate-50">
    <GlobalSidebar page={page} navigate={navigate} open={mobileNav} onClose={() => setMobileNav(false)} />
    <main className="min-h-screen lg:pl-64">
      <div className="lg:hidden flex h-14 items-center justify-between border-b border-slate-200 bg-white px-4"><Brand/><MobileMenuButton open={() => setMobileNav(true)} /></div>
      {workspace && <IndexHeader current={workspace} index={index!} navigate={navigate} />}
      <div className="mx-auto max-w-[1680px] px-4 py-6 sm:px-6 lg:px-8">
        {Boolean(error) && index && <QueryErrorBanner error={error} retry={() => { void refetch() }} />}
        {capabilities.error && <QueryErrorBanner error={capabilities.error} retry={() => { void capabilities.retry() }} />}
        {workspace && <div className="mb-4 flex items-center gap-2 rounded-lg border border-indigo-100 bg-indigo-50 px-3 py-2 text-xs text-indigo-900"><InformationCircleIcon className="size-4 shrink-0"/>Static preview fixture for <code>{index?.id}</code>. Values and explanations are representative, not daemon output.</div>}
        <IndexContext.Provider value={index ?? null}>{workspace && navigation.sections[workspace].length > 0 ? <div className="grid gap-6 lg:grid-cols-[180px_minmax(0,1fr)]"><SectionNavigation tab={workspace} page={page} navigate={navigate}/><Page page={page} navigate={navigate}/></div> : <Page page={page} navigate={navigate}/>}</IndexContext.Provider>
      </div>
    </main>
    {commandOpen && <CommandPalette close={() => setCommandOpen(false)} navigate={navigate} />}
  </div>
}
