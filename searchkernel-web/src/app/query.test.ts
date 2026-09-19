import { describe, expect, it } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { SearchKernelClientError } from '../api/searchkernel-client'
import { clientQueryDefaults } from './query'

describe('query defaults', () => {
  it('keeps the last successful value when a refetch fails', async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const queryKey = ['stale-value']
    await queryClient.fetchQuery({ queryKey, queryFn: async () => 'successful response' })
    await expect(queryClient.fetchQuery({ queryKey, queryFn: async () => { throw new SearchKernelClientError('temporary outage', { retryable: true }) }, staleTime: 0, retry: clientQueryDefaults.retry })).rejects.toThrow('temporary outage')
    expect(queryClient.getQueryData(queryKey)).toBe('successful response')
  })
})
