import { describe, it, expect } from 'vitest'
import { prepareSearch, typedTerm, preparedRequest } from '../../api/search-request'
import { toCanonicalJson } from '@searchkernel/client'
import { decodeSearchParams, encodeSearchParams, updateSearchState } from './search-params'
import type { ClientSearchRequest } from '../../api/searchkernel-client'

const combination: ClientSearchRequest = { query: 'orchid', mode: 'typed', typedQuery: typedTerm('title', 'orchid'), filters: [{ field: 'quantity', operator: 'gte', value: '7' }, { field: 'quantity', operator: 'in', value: '[7,11]' }], sorts: [{ field: 'quantity', direction: 'desc' }, { field: 'title', direction: 'asc' }], offset: 2, limit: 2, ranking: 'boosted', inputs: '{"boost":2,"large":{"kind_type":"Int64Value","kind":{"value":"9007199254740993"}}}', summary: 'compact' }

describe('Search Lab reproducible workflow', () => {
  it('round trips representative combinations and uses deterministic URL ordering', () => {
    const url = encodeSearchParams(combination)
    expect(decodeSearchParams(url)).toEqual({ state: combination })
    expect(encodeSearchParams({ ...combination }).toString()).toBe(url.toString())
    expect(encodeSearchParams(Object.fromEntries(Object.entries(combination).reverse()) as ClientSearchRequest).toString()).toBe(url.toString())
    expect(encodeSearchParams(decodeSearchParams(url).state).toString()).toBe(url.toString())
  })
  it('inspects the exact object prepared for execution, preserving all controls and types', () => {
    const prepared = prepareSearch(combination), request = JSON.parse(prepared.canonical)
    expect(JSON.parse(prepared.canonical)).toEqual(toCanonicalJson(preparedRequest(prepared)))
    expect(request.query.source_type).toBe('TypedQuery')
    expect(request.filters.structured).toHaveLength(2)
    expect(request.sort.map((s: any) => s.target.target.field)).toEqual(['quantity', 'title'])
    expect(request.ranking.profile).toBe('boosted')
    expect(request.offset.value).toBe(2)
    expect(request.limit.value).toBe(2)
    expect(request.inputs.find((i: any) => i.name === 'large').input.value.kind.value).toBe('9007199254740993')
    expect(request.presentation.summary).toBe('compact')
  })
  it('leaves omitted controls absent and matches text/browse canonical sources', () => {
    expect(JSON.parse(prepareSearch({ query: 'orchid' }).canonical)).toMatchObject({ query: { source_type: 'SearchKernelTextQuery', source: { text: 'orchid' } } })
    const browse = JSON.parse(prepareSearch({ query: '' }).canonical)
    expect(browse.query.source_type).toBe('MatchAllQuery')
    expect(browse.limit).toBeUndefined(); expect(browse.ranking).toBeUndefined(); expect(browse.offset).toBeUndefined()
  })
  it('resets pagination for every semantic edit and preserves explicit page movement', () => {
    for (const patch of [{ query: 'tulip' }, { typedQuery: typedTerm('title', 'tulip') }, { filters: [] }, { sorts: [] }, { ranking: 'other' }, { inputs: '{}' }, { limit: 10 }, { summary: 'other' }, { mode: 'text' as const }]) expect(updateSearchState(combination, patch).offset).toBeUndefined()
    expect(updateSearchState(combination, { offset: 4 }).offset).toBe(4)
  })
  it('handles malformed URLs without executing their contents, and retains invalid editor text for correction', () => {
    for (const s of ['bad', '{}', '{"v":2,"state":{}}', '{"v":1,"state":{"query":9}}', '{"v":1,"state":{"query":"","filters":[null]}}', '{"v":1,"state":{"query":"","sorts":[{"field":"x","direction":"sideways"}]}}']) {
      expect(decodeSearchParams(new URLSearchParams({ s }))).toMatchObject({ state: { query: '' }, error: expect.any(String) })
    }
    const invalid = { query: 'orchid', inputs: '{unfinished' }
    expect(decodeSearchParams(encodeSearchParams(invalid)).state).toEqual(invalid)
    expect(() => prepareSearch(invalid)).toThrow()
    expect(() => prepareSearch({ ...combination, filters: [{ field: 'quantity', operator: 'in', value: '7' }] })).toThrow('array')
  })
})
