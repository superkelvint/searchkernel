import type { Capabilities } from '../api/searchkernel-client'

export type Page = 'global-overview' | 'indexes' | 'index-overview' | 'documents' | 'search' | 'analyze' | 'inspect' | 'schema' | 'ranking' | 'summaries' | 'health' | 'diagnostics' | 'operations' | 'settings' | 'integrations'
export type Tab = 'Overview' | 'Data' | 'Search' | 'Configuration' | 'Operations' | 'Integrations'
export type Nav = (page: Page, indexId?: string) => void
export type NavigationItem = { label: string; page: Page; visible: boolean; enabled: boolean; reason?: string }
export type NavigationModel = { global: NavigationItem[]; tabs: NavigationItem[]; sections: Record<Tab, NavigationItem[]> }

const indexPages = new Set<Page>(['index-overview', 'documents', 'search', 'analyze', 'inspect', 'schema', 'ranking', 'summaries', 'health', 'diagnostics', 'integrations'])

export function isIndexPage(page: Page) { return indexPages.has(page) }

export function pagePath(page: Page, indexId?: string) {
  if (page === 'global-overview') return '/overview'
  if (page === 'indexes') return '/indexes'
  if (page === 'operations') return '/operations'
  if (page === 'settings') return '/settings'
  if (!indexId) throw new Error(`An index id is required for ${page}.`)
  return {
    integrations: `/indexes/${indexId}/integrations/instantsearch`, 'index-overview': `/indexes/${indexId}/overview`, documents: `/indexes/${indexId}/data/documents`, search: `/indexes/${indexId}/search/lab`,
    analyze: `/indexes/${indexId}/search/analyze`, inspect: `/indexes/${indexId}/search/inspect`, schema: `/indexes/${indexId}/configuration/schema/fields`,
    ranking: `/indexes/${indexId}/configuration/ranking`, summaries: `/indexes/${indexId}/configuration/summaries`, health: `/indexes/${indexId}/operations/health`, diagnostics: `/indexes/${indexId}/operations/diagnostics`,
  }[page as Exclude<Page, 'global-overview' | 'indexes' | 'operations' | 'settings'>]
}

const requirementFor: Partial<Record<Page, (capabilities: Capabilities) => boolean>> = {
  'global-overview': capabilities => capabilities.indexes.read,
  indexes: capabilities => capabilities.indexes.read,
  integrations: capabilities => capabilities.indexes.read,
  'index-overview': capabilities => capabilities.indexes.read,
  documents: capabilities => capabilities.documents.read,
  search: capabilities => capabilities.search.basic,
  analyze: capabilities => capabilities.analysis.text,
  inspect: capabilities => capabilities.query.inspect,
  schema: capabilities => capabilities.schema.read,
  ranking: capabilities => capabilities.ranking.profiles,
  summaries: capabilities => capabilities.summaries.read,
  health: capabilities => capabilities.admin.health,
  diagnostics: capabilities => capabilities.admin.diagnostics,
  operations: capabilities => capabilities.admin.overview,
}

const staticNavigation: { global: [string, Page][]; sections: Record<Tab, [string, Page][]> } = {
  global: [['Overview', 'global-overview'], ['Indexes', 'indexes'], ['Operations', 'operations'], ['Settings', 'settings']],
  sections: {
    Overview: [],
    Integrations: [['InstantSearch.js', 'integrations']],
    Data: [['Documents', 'documents']],
    Search: [['Search Lab', 'search'], ['Analyze', 'analyze'], ['Query Inspector', 'inspect']],
    Configuration: [['Schema', 'schema'], ['Ranking', 'ranking'], ['Summaries', 'summaries']],
    Operations: [['Health', 'health'], ['Diagnostics', 'diagnostics']],
  },
}

function item(label: string, page: Page, capabilities?: Capabilities): NavigationItem {
  if (!capabilities) return { label, page, visible: true, enabled: false, reason: 'Capabilities are loading.' }
  const requirement = requirementFor[page]
  return requirement && !requirement(capabilities) ? { label, page, visible: false, enabled: false, reason: 'This capability is not exposed by the server.' } : { label, page, visible: true, enabled: true }
}

export function navigationFor(capabilities?: Capabilities): NavigationModel {
  const sections = Object.fromEntries(Object.entries(staticNavigation.sections).map(([tab, entries]) => [tab, entries.map(([label, page]) => item(label, page, capabilities))])) as Record<Tab, NavigationItem[]>
  return {
    global: staticNavigation.global.map(([label, page]) => item(label, page, capabilities)),
    tabs: [item('Overview', 'index-overview', capabilities), ...(['Data', 'Search', 'Configuration', 'Operations', 'Integrations'] as const).map(tab => ({ label: tab, page: ({ Data: 'documents', Search: 'search', Configuration: 'schema', Operations: 'health', Integrations: 'integrations' }[tab]) as Page, visible: sections[tab].some(entry => entry.visible), enabled: sections[tab].some(entry => entry.enabled), reason: sections[tab].find(entry => entry.visible && !entry.enabled)?.reason ?? sections[tab].find(entry => !entry.visible)?.reason }))],
    sections,
  }
}

export const tabFor = (page: Page): Tab | null => {
  const tabs: Partial<Record<Page, Tab>> = { integrations: 'Integrations', 'index-overview': 'Overview', documents: 'Data', search: 'Search', analyze: 'Search', inspect: 'Search', schema: 'Configuration', ranking: 'Configuration', summaries: 'Configuration', health: 'Operations', diagnostics: 'Operations' }
  return tabs[page] ?? null
}
