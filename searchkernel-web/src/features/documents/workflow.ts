import { SearchKernelClientError, normalizeClientError, type SearchKernelClient, type PreparedDocumentMutation, type PreparedSearchRequest, type RequestOptions } from '../../api/searchkernel-client'

/** Dispatch one write, then perform cancellable read-back outside browser caches.
 *
 * A transport/protocol failure after dispatch cannot prove that the daemon did
 * not commit the mutation: searchkerneld may already be executing the blocking
 * canonical dispatch. Such failures are deliberately non-retryable until an
 * authoritative GET reconciles the document state.
 */
export async function mutateAndVerify(client: SearchKernelClient, target: string, mutation: PreparedDocumentMutation, search: PreparedSearchRequest, options?: RequestOptions) {
  let acknowledgement
  try {
    // Do not bind the write lifetime to a React observer/component AbortSignal.
    // Once dispatched, cancellation cannot safely distinguish "not applied"
    // from "applied but response lost".
    acknowledgement = await client.executeDocumentMutation(target, mutation)
  } catch (error) {
    const cause = normalizeClientError(error)
    if (cause.origin === 'canonical' || cause.origin === 'client') throw cause
    throw new SearchKernelClientError(`Mutation outcome is unknown because the response was not received safely: ${cause.message}. Read the current document before repeating this write.`, {
      code: 'mutation_outcome_unknown',
      origin: cause.origin,
      status: cause.status,
      retryable: false,
      details: {
        outcome: 'unknown',
        operation: mutation.operation,
        id: mutation.id,
        cause: { code: cause.code, origin: cause.origin ?? null, retryable: cause.retryable, status: cause.status ?? null, details: cause.details ?? null },
      },
    })
  }
  try {
    const [document, response] = await Promise.all([client.getDocument(target, mutation.id, options), client.executeSearch(target, search, options)])
    if ((mutation.operation === 'delete' && document !== undefined) || (mutation.operation !== 'delete' && document === undefined)) throw new SearchKernelClientError('Acknowledged mutation violated immediate GET visibility', { code: 'visibility_error', origin: 'protocol' })
    return {
      acknowledgement,
      document,
      search: response,
      writeOutcome: mutation.operation === 'delete' && acknowledgement.deleted === false ? 'not_deleted' as const : 'applied' as const,
      readbackPresence: document === undefined ? 'absent' as const : 'present' as const,
    }
  } catch (error) {
    const cause = normalizeClientError(error)
    throw new SearchKernelClientError(`Mutation acknowledged, but fresh read-back failed: ${cause.message}. Refresh reads before repeating a write.`, { code: cause.code, origin: cause.origin, status: cause.status, retryable: false, details: { acknowledged: true, operation: mutation.operation, id: mutation.id, acknowledgement: acknowledgement.canonicalResponse, cause: { code: cause.code, origin: cause.origin ?? null, retryable: cause.retryable, status: cause.status ?? null, details: cause.details ?? null } } })
  }
}

// An explicit by-id route uses opaque UTF-8 tokens. This avoids router percent /
// slash decoding ambiguities; legacy document routes retain their router-decoded ID.
export function encodeDocumentId(id: string): string { return 'u.' + btoa(Array.from(new TextEncoder().encode(id), byte => String.fromCharCode(byte)).join('')).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '') }
export function decodeDocumentId(token: string): string {
  if (!/^u\.[A-Za-z0-9_-]+$/.test(token)) throw new TypeError('Malformed document URL token')
  const id = new TextDecoder('utf-8', { fatal: true }).decode(Uint8Array.from(atob(token.slice(2).replaceAll('-', '+').replaceAll('_', '/')), char => char.charCodeAt(0)))
  if (encodeDocumentId(id) !== token) throw new TypeError('Noncanonical document URL token')
  return id
}
export function documentPath(target: string, id: string) { return `/indexes/${encodeURIComponent(target)}/data/documents/by-id/${encodeDocumentId(id)}` }
