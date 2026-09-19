import type { SearchKernelClient } from '../api/searchkernel-client'
import { type AdminClient, type AdminIndexSummary, type AdminOverview, type Diagnostics, type Health } from './admin-client'

const summaries: AdminIndexSummary[] = [
  { id: 'products', documentCount: '2,482,192', size: '3.8 GB', fieldCount: '5', latency: '3.7 ms' },
  { id: 'articles', documentCount: '81,402', size: '440 MB', fieldCount: '3', latency: '2.9 ms' },
  { id: 'users', documentCount: '12,832', size: '91 MB', fieldCount: '3', latency: '1.8 ms' },
]

export function createFixtureAdminClient(searchKernelClient: SearchKernelClient): AdminClient {
  return {
    async getOverview(): Promise<AdminOverview> {
      const indexes = await searchKernelClient.listIndexes()
      return { indexCount: indexes.length, documentCount: '2.6m', storage: '4.4 GB', daemon: 'Disconnected' }
    },
    async listIndexSummaries(): Promise<AdminIndexSummary[]> {
      return summaries
    },
    async getIndexSummary(indexId: string): Promise<AdminIndexSummary | undefined> {
      return summaries.find(summary => summary.id === indexId)
    },
    async getHealth(indexId: string): Promise<Health | undefined> {
      const index = await searchKernelClient.getIndex(indexId)
      const summary = summaries.find(candidate => candidate.id === indexId)
      return index && summary && { status: 'Fixture state', message: 'Live health and warnings will be supplied by the index metrics API.', documents: summary.documentCount, size: summary.size, fields: summary.fieldCount, latency: summary.latency }
    },
    async getDiagnostics(indexId: string): Promise<Diagnostics | undefined> {
      const index = await searchKernelClient.getIndex(indexId)
      return index && { mode: 'static fixture', index: index.id, serverConnection: 'none', documentContents: 'omitted', secrets: 'omitted', filesystemPaths: 'omitted' }
    },
  }
}
