import { CreateIndex } from '../features/configuration/CreateIndex'
import { BrowserRouter, Navigate, Route, Routes, useParams, useLocation } from 'react-router-dom'
import { DashboardRoute } from './DashboardRoute'
import { About, NotFound, UnavailableFeature } from './fallback-pages'

function IndexDefault({section}:{section?: 'data'|'search'|'configuration'|'operations'}) {
  const { indexId } = useParams()
  const { search } = useLocation()
  const suffix = section === 'data' ? 'data/documents' : section === 'search' ? 'search/lab' : section === 'configuration' ? 'configuration/schema/fields' : section === 'operations' ? 'operations/health' : 'overview'
  return <Navigate replace to={`/indexes/${indexId}/${suffix}${search}`}/>
}

export function AppRoutes() { return <Routes>
  <Route path="/" element={<Navigate replace to="/overview"/>}/>
  <Route path="/overview" element={<DashboardRoute page="global-overview"/>}/>
  <Route path="/indexes" element={<DashboardRoute page="indexes"/>}/>
  <Route path="/indexes/new" element={<CreateIndex/>}/>
  <Route path="/indexes/:indexId" element={<IndexDefault/>}/>
  <Route path="/indexes/:indexId/data" element={<IndexDefault section="data"/>}/>
  <Route path="/indexes/:indexId/search" element={<IndexDefault section="search"/>}/>
  <Route path="/indexes/:indexId/configuration" element={<IndexDefault section="configuration"/>}/>
  <Route path="/indexes/:indexId/operations" element={<IndexDefault section="operations"/>}/>
  <Route path="/indexes/:indexId/overview" element={<DashboardRoute page="index-overview"/>}/>
  <Route path="/indexes/:indexId/data/documents" element={<DashboardRoute page="documents"/>}/>
  <Route path="/indexes/:indexId/data/documents/by-id/:documentToken" element={<DashboardRoute page="documents"/>}/>
  <Route path="/indexes/:indexId/data/documents/:documentId" element={<DashboardRoute page="documents"/>}/>
  <Route path="/indexes/:indexId/search/lab" element={<DashboardRoute page="search"/>}/>
  <Route path="/indexes/:indexId/search/analyze" element={<DashboardRoute page="analyze"/>}/>
  <Route path="/indexes/:indexId/search/inspect" element={<DashboardRoute page="inspect"/>}/>
  <Route path="/indexes/:indexId/configuration/schema" element={<Navigate replace to="fields"/>}/>
  <Route path="/indexes/:indexId/configuration/schema/fields" element={<DashboardRoute page="schema"/>}/>
  <Route path="/indexes/:indexId/configuration/schema/indexing" element={<UnavailableFeature title="Indexing"/>}/>
  <Route path="/indexes/:indexId/configuration/schema/fieldsets" element={<UnavailableFeature title="Fieldsets"/>}/>
  <Route path="/indexes/:indexId/configuration/ranking" element={<DashboardRoute page="ranking"/>}/>
  <Route path="/indexes/:indexId/configuration/ranking/:profile" element={<DashboardRoute page="ranking"/>}/>
  <Route path="/indexes/:indexId/configuration/summaries" element={<DashboardRoute page="summaries"/>}/>
  <Route path="/indexes/:indexId/configuration/summaries/:summary" element={<DashboardRoute page="summaries"/>}/>
  <Route path="/indexes/:indexId/configuration/search-behavior" element={<UnavailableFeature title="Search Behavior"/>}/>
  <Route path="/indexes/:indexId/configuration/assets" element={<UnavailableFeature title="Assets"/>}/>
  <Route path="/indexes/:indexId/operations/health" element={<DashboardRoute page="health"/>}/>
  <Route path="/indexes/:indexId/operations/query-performance" element={<UnavailableFeature title="Query Performance"/>}/>
  <Route path="/indexes/:indexId/operations/storage" element={<UnavailableFeature title="Indexing & Storage"/>}/>
  <Route path="/indexes/:indexId/operations/diagnostics" element={<DashboardRoute page="diagnostics"/>}/>
  <Route path="/operations" element={<DashboardRoute page="operations"/>}/><Route path="/settings" element={<DashboardRoute page="settings"/>}/><Route path="/about" element={<About/>}/>
  <Route path="*" element={<NotFound/>}/>
</Routes> }

export function App() { return <BrowserRouter><AppRoutes/></BrowserRouter> }
