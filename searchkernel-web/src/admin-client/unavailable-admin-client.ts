import type { AdminClient } from './admin-client'

/** Operational metrics have no daemon API in this slice. */
export function createUnavailableAdminClient(): AdminClient {
  return {
    async getOverview() { return { documentCount: 'Unavailable', storage: 'Unavailable', daemon: 'Unavailable' } },
    async listIndexSummaries() { return [] },
    async getIndexSummary() { return undefined },
    async getHealth() { return undefined },
    async getDiagnostics() { return undefined },
  }
}
