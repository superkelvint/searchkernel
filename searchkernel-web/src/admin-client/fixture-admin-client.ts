import type { SearchKernelClient } from '../api/searchkernel-client'
import { type AdminClient, type AdminOverview, type Diagnostics, type Health } from './admin-client'

export function createFixtureAdminClient(searchKernelClient: SearchKernelClient): AdminClient {
  return {
    async getOverview(): Promise<AdminOverview> {
      const indexes = await searchKernelClient.listIndexes()
      return { indexCount: indexes.length, documentCount: '2.6m', storage: '4.4 GB', daemon: 'Disconnected' }
    },
    async getHealth(indexId: string): Promise<Health | undefined> {
      const index = await searchKernelClient.getIndex(indexId)
      return index && { status: 'Fixture state', message: 'Live health and warnings will be supplied by the index metrics API.', documents: index.documentCount, size: index.size, fields: index.fieldCount, latency: index.latency }
    },
    async getDiagnostics(indexId: string): Promise<Diagnostics | undefined> {
      const index = await searchKernelClient.getIndex(indexId)
      return index && { mode: 'static fixture', index: index.id, serverConnection: 'none', documentContents: 'omitted', secrets: 'omitted', filesystemPaths: 'omitted' }
    },
  }
}
