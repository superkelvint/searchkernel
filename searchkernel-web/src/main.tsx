import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { createFixtureSearchKernelClient } from './api/fixture-searchkernel-client'
import { createFixtureAdminClient } from './admin-client/fixture-admin-client'
import { createDaemonSearchKernelClient } from './api/daemon-searchkernel-client'
import { createUnavailableAdminClient } from './admin-client/unavailable-admin-client'
import { ClientProvider } from './app/client-context'
import { AppErrorBoundary } from './app/AppErrorBoundary'
import './styles.css'

const client = new QueryClient({ defaultOptions: { queries: { staleTime: 30_000, retry: (failureCount, error) => { const normalized = error as { retryable?: boolean }; return normalized.retryable === true && failureCount < 2 } } } })
const demo = import.meta.env.VITE_SEARCHKERNEL_MODE === 'fixture'
const searchKernelClient = demo ? createFixtureSearchKernelClient() : createDaemonSearchKernelClient(import.meta.env.VITE_SEARCHKERNEL_ENDPOINT ?? 'http://localhost:8081')
const adminClient = demo ? createFixtureAdminClient(searchKernelClient) : createUnavailableAdminClient()
createRoot(document.getElementById('root')!).render(<StrictMode><AppErrorBoundary><QueryClientProvider client={client}><ClientProvider searchKernelClient={searchKernelClient} adminClient={adminClient}><App /></ClientProvider></QueryClientProvider></AppErrorBoundary></StrictMode>)
