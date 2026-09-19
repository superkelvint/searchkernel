import type { Capabilities } from '../api/searchkernel-client'

export type Page = 'global-overview' | 'indexes' | 'index-overview' | 'documents' | 'search' | 'analyze' | 'inspect' | 'schema' | 'ranking' | 'summaries' | 'health' | 'diagnostics' | 'operations' | 'settings'
export type Tab = 'Overview' | 'Data' | 'Search' | 'Configuration' | 'Operations'
export type Nav = (page: Page, indexId?: string) => void
export type NavigationItem = { label: string; page: Page; enabled: boolean; reason?: string }
export type NavigationModel = { global: NavigationItem[]; tabs: NavigationItem[]; sections: Record<Tab, NavigationItem[]> }

export const pagePath = (page: Page, indexId = 'products') => ({
  'global-overview': '/overview', indexes: '/indexes', 'index-overview': `/indexes/${indexId}/overview`, documents: `/indexes/${indexId}/data/documents`,
  search: `/indexes/${indexId}/search/lab`, analyze: `/indexes/${indexId}/search/analyze`, inspect: `/indexes/${indexId}/search/inspect`,
  schema: `/indexes/${indexId}/configuration/schema/fields`, ranking: `/indexes/${indexId}/configuration/ranking`, summaries: `/indexes/${indexId}/configuration/summaries`,
  health: `/indexes/${indexId}/operations/health`, diagnostics: `/indexes/${indexId}/operations/diagnostics`, operations: '/operations', settings: '/settings',
})[page]

const requirementFor: Partial<Record<Page, (capabilities: Capabilities) => boolean>> = {
  'global-overview': capabilities => capabilities.indexes.read,
  indexes: capabilities => capabilities.indexes.read,
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
    Data: [['Documents', 'documents']],
    Search: [['Search Lab', 'search'], ['Analyze', 'analyze'], ['Query Inspector', 'inspect']],
    Configuration: [['Schema', 'schema'], ['Ranking', 'ranking'], ['Summaries', 'summaries']],
    Operations: [['Health', 'health'], ['Diagnostics', 'diagnostics']],
  },
}

function item(label: string, page: Page, capabilities?: Capabilities): NavigationItem {
  if (!capabilities) return { label, page, enabled: false, reason: 'Capabilities are loading.' }
  const requirement = requirementFor[page]
  return requirement && !requirement(capabilities) ? { label, page, enabled: false, reason: 'This capability is not exposed by the server.' } : { label, page, enabled: true }
}

export function navigationFor(capabilities?: Capabilities): NavigationModel {
  const sections = Object.fromEntries(Object.entries(staticNavigation.sections).map(([tab, entries]) => [tab, entries.map(([label, page]) => item(label, page, capabilities))])) as Record<Tab, NavigationItem[]>
  return {
    global: staticNavigation.global.map(([label, page]) => item(label, page, capabilities)),
    tabs: [item('Overview', 'index-overview', capabilities), { label: 'Data', page: 'documents', enabled: sections.Data.some(entry => entry.enabled), reason: sections.Data.find(entry => !entry.enabled)?.reason }, { label: 'Search', page: 'search', enabled: sections.Search.some(entry => entry.enabled), reason: sections.Search.find(entry => !entry.enabled)?.reason }, { label: 'Configuration', page: 'schema', enabled: sections.Configuration.some(entry => entry.enabled), reason: sections.Configuration.find(entry => !entry.enabled)?.reason }, { label: 'Operations', page: 'health', enabled: sections.Operations.some(entry => entry.enabled), reason: sections.Operations.find(entry => !entry.enabled)?.reason }],
    sections,
  }
}

export const tabFor = (page: Page): Tab | null => {
  const tabs: Partial<Record<Page, Tab>> = { 'index-overview': 'Overview', documents: 'Data', search: 'Search', analyze: 'Search', inspect: 'Search', schema: 'Configuration', ranking: 'Configuration', summaries: 'Configuration', health: 'Operations', diagnostics: 'Operations' }
  return tabs[page] ?? null
}
