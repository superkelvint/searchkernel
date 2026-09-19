import { normalizeClientError, type SearchKernelClientError } from '../api/searchkernel-client'
import type { QueryFunctionContext } from '@tanstack/react-query'

export const clientQueryDefaults = {
  staleTime: 30_000,
  retry: (failureCount: number, error: unknown) => normalizeClientError(error).retryable && failureCount < 2,
}

export function clientQuery<T>(read: (signal: AbortSignal) => Promise<T>) {
  return async ({ signal }: QueryFunctionContext) => {
    try { return await read(signal) } catch (error) { throw normalizeClientError(error) }
  }
}

export function queryError(error: unknown): SearchKernelClientError | undefined {
  return error ? normalizeClientError(error) : undefined
}
