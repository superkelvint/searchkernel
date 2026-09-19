import { createContext, useContext } from 'react'
import type { Index } from '../api/searchkernel-client'

const IndexContext = createContext<Index | null>(null)

export { IndexContext }

export function useIndex() {
  const index = useContext(IndexContext)
  if (!index) throw new Error('Index is unavailable outside an index workspace.')
  return index
}

export const useFixture = useIndex
