export type AdminOverview = {
  indexCount?: number
  documentCount: string
  storage: string
  daemon: string
}
export type AdminIndexSummary = { id: string; documentCount: string; size: string; fieldCount: string; latency: string }
export type Health = { status: string; message: string; documents: string; size: string; fields: string; latency: string }
export type Diagnostics = { mode: string; index: string; serverConnection: string; documentContents: string; secrets: string; filesystemPaths: string }

/** Browser boundary for operational/admin endpoints, separate from portable operations. */
export interface AdminClient {
  getOverview(): Promise<AdminOverview>
  listIndexSummaries(): Promise<AdminIndexSummary[]>
  getIndexSummary(indexId: string): Promise<AdminIndexSummary | undefined>
  getHealth(indexId: string): Promise<Health | undefined>
  getDiagnostics(indexId: string): Promise<Diagnostics | undefined>
}
