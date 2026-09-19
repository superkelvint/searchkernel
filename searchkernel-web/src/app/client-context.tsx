import { createContext, useContext, type ReactNode } from 'react'
import { useQuery } from '@tanstack/react-query'
import { type Capabilities, type SearchKernelClient } from '../api/searchkernel-client'
import type { AdminClient } from '../admin-client/admin-client'
import { clientQuery, clientQueryDefaults, queryError } from './query'

const SearchKernelClientContext = createContext<SearchKernelClient | null>(null)
const AdminClientContext = createContext<AdminClient | null>(null)
type CapabilitiesState = { capabilities?: Capabilities; loading: boolean; error?: ReturnType<typeof queryError>; retry: () => Promise<unknown> }
const CapabilitiesContext = createContext<CapabilitiesState | null>(null)

export function ClientProvider({ searchKernelClient, adminClient, children }: { searchKernelClient: SearchKernelClient; adminClient: AdminClient; children: ReactNode }) {
  return <SearchKernelClientContext.Provider value={searchKernelClient}><AdminClientContext.Provider value={adminClient}><CapabilitiesProvider>{children}</CapabilitiesProvider></AdminClientContext.Provider></SearchKernelClientContext.Provider>
}

function CapabilitiesProvider({ children }: { children: ReactNode }) {
  const client = useSearchKernelClient()
  const result = useQuery({ queryKey: ['capabilities'], queryFn: clientQuery(() => client.capabilities()), ...clientQueryDefaults, staleTime: 300_000 })
  return <CapabilitiesContext.Provider value={{ capabilities: result.data, loading: result.isLoading, error: queryError(result.error), retry: result.refetch }}>{children}</CapabilitiesContext.Provider>
}

export function useSearchKernelClient() {
  const client = useContext(SearchKernelClientContext)
  if (!client) throw new Error('SearchKernelClient is unavailable outside ClientProvider.')
  return client
}

export function useAdminClient() {
  const client = useContext(AdminClientContext)
  if (!client) throw new Error('AdminClient is unavailable outside ClientProvider.')
  return client
}

export function useCapabilities() {
  const state = useContext(CapabilitiesContext)
  if (!state) throw new Error('Capabilities are unavailable outside ClientProvider.')
  return state
}
