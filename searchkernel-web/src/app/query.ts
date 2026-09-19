import { normalizeClientError, type SearchKernelClientError } from '../api/searchkernel-client'

export const clientQueryDefaults = {
  staleTime: 30_000,
  retry: (failureCount: number, error: unknown) => normalizeClientError(error).retryable && failureCount < 2,
}

export function clientQuery<T>(read: () => Promise<T>) {
  return async () => {
    try { return await read() } catch (error) { throw normalizeClientError(error) }
  }
}

export function queryError(error: unknown): SearchKernelClientError | undefined {
  return error ? normalizeClientError(error) : undefined
}
