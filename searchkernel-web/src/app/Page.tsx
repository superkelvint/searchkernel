import { Integrations } from '../features/integrations/Integrations'
import type { Nav, Page as PageName } from './navigation'
import { GlobalOverview } from '../features/overview/pages'
import { Indexes, IndexOverview } from '../features/indexes/Pages'
import { Documents } from '../features/documents/Documents'
import { Analyze, QueryInspector } from '../features/search-debugging/Pages'
import { SearchLab } from '../features/search-lab/SearchLab'
import { Schema } from '../features/configuration/Schema'
import { Ranking } from '../features/ranking/Ranking'
import { Summaries } from '../features/summaries/Summaries'
import { Diagnostics, GlobalOperations, Health } from '../features/operations/Operations'
import { Settings } from '../features/settings/Settings'

export function Page({page,navigate}:{page:PageName,navigate:Nav}) { switch(page) {case 'integrations':return <Integrations/>;case 'global-overview':return <GlobalOverview navigate={navigate}/>;case 'indexes':return <Indexes navigate={navigate}/>;case 'index-overview':return <IndexOverview navigate={navigate}/>;case 'documents':return <Documents/>;case 'search':return <SearchLab/>;case 'analyze':return <Analyze/>;case 'inspect':return <QueryInspector/>;case 'schema':return <Schema/>;case 'ranking':return <Ranking/>;case 'summaries':return <Summaries/>;case 'health':return <Health/>;case 'diagnostics':return <Diagnostics/>;case 'operations':return <GlobalOperations/>;case 'settings':return <Settings/>} }
