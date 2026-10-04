import { normalizeClientError, type IndexTargetT } from '../../api/searchkernel-client'

export { reconcileTarget } from '../../api/searchkernel-client'

export function canDelete(target: IndexTargetT, confirmation: string, running: boolean): boolean {
  return !!target.generation && confirmation === target.name && !running
}

export function failureAction(error: unknown): string {
  const value = normalizeClientError(error)
  const details = JSON.stringify(value.details)
  if (details?.includes('stale_index_handle')) return 'This handle no longer refers to the active incarnation. It may have been deleted/recreated or reopened after restart. Continuing with this handle is unsafe. Refresh inventory and explicitly open/select the current incarnation.'
  if (value.origin === 'transport') return 'Check the daemon connection, refresh health, and retry reads. A lost create/delete response has an unknown outcome: refresh inventory before repeating it.'
  if (value.code === 'already_exists') return 'Refresh inventory and open the existing index, or choose a different name.'
  if (value.code === 'not_found') return 'Refresh inventory. If the index is persisted, open it; otherwise create a new index.'
  if (value.code === 'invalid_argument') return 'Return to the definition editor and validate the canonical definition.'
  if (details?.includes('lock')) return 'Another lifecycle operation owns this index. Wait for it to finish and refresh.'
  if (value.code === 'failed_precondition') return 'Inspect runtime and index diagnostics. Refresh before retrying the lifecycle operation.'
  return 'Inspect the canonical error details and daemon/index diagnostics. Refresh authoritative state before retrying.'
}
