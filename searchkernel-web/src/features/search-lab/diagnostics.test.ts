import { describe, expect, it } from 'vitest'
import { toCanonicalJson } from '@searchkernel/client'
import { prepareSearch } from '../../api/search-request'
import { prepareDiagnostics, preparedDiagnosticRequest } from '../../api/diagnostic-request'
import type { ClientDiagnostics, Index } from '../../api/searchkernel-client'
import { diagnosticBundle, signalComparison } from './diagnostic-export'
import { encodeSearchParams } from './search-params'
import { diagnosticCurl } from '../../api/diagnostic-request'

const index: Index = { id: 'products', generation: 'G1', profiles: [], summaries: [], fields: [], searchDefaults: {} }
describe('canonical diagnostic construction and export', () => {
  it('keeps query feature values out of default shared URLs and curl rejects credential-bearing endpoints', () => {
    const state = { query: 'orchid', inputs: '{"PRIVATE_FEATURE":42}' }
    expect(encodeSearchParams(state, { includeInputs: false }).toString()).not.toContain('PRIVATE_FEATURE')
    expect(encodeSearchParams(state, { includeInputs: true }).toString()).toContain('PRIVATE_FEATURE')
    const prepared = prepareDiagnostics(index, prepareSearch(state), 'source', { intent: 'explain-hit' })
    expect(diagnosticCurl(prepared, 'http://localhost:8080')).toContain('/v1/execute')
    expect(diagnosticCurl(prepared, 'http://localhost:8080')).toContain('G1')
    expect(() => diagnosticCurl(prepared, 'http://user:password@localhost:8080')).toThrow('credentials')
    expect(() => diagnosticCurl(prepared, 'http://localhost:8080?token=secret')).toThrow('credentials')
  })
  it('uses the exact W1 request and preserves typed inputs, filters, profile and pagination', () => {
    const query = prepareSearch({ query: 'orchid', ranking: 'lexical', limit: 10, offset: 20,
      filters: [{ field: 'quantity', operator: 'gte', value: '7' }],
      inputs: '{"boost":2,"embedding":{"kind_type":"TensorValue","kind":{"type_spec":"tensor(x[2])","payload_type":"DenseTensorValues","payload":{"values":[1,2]}}}}' })
    const diagnostic = prepareDiagnostics(index, query, 'source', { intent: 'explain-hit', selectedHits: ['a'], sections: ['ranking'], timeoutMs: 5000 })
    const exact = preparedDiagnosticRequest(diagnostic)
    expect(JSON.parse(diagnostic.canonical)).toEqual(toCanonicalJson(exact))
    expect(JSON.parse(diagnostic.canonical).request).toEqual(JSON.parse(query.canonical))
    expect(diagnostic).toMatchObject({ target: 'products', generation: 'G1' })
    expect(exact.sourceRunId).toBe('source')
    expect(exact.limits?.timeoutMs?.value).toBe(5000n)
    expect(() => preparedDiagnosticRequest({ ...diagnostic })).toThrow('prepared')
  })
  it('does not construct zero-result variants in the browser or mutate the original request', () => {
    const original = prepareSearch({ query: 'orchid', filters: [{ field: 'in_stock', operator: 'eq', value: 'true' }] })
    const before = original.canonical
    const request = prepareDiagnostics(index, original, undefined, { intent: 'zero-results' })
    expect(JSON.parse(request.canonical).request).toEqual(JSON.parse(before))
    expect(original.canonical).toBe(before)
    expect(() => prepareDiagnostics({ ...index, generation: undefined }, original, undefined, { intent: 'explain-hit' })).toThrow('Open the index')
  })
  it('keeps experimental canonical requests separate from the baseline', () => {
    const original = prepareSearch({ query: 'orchid', ranking: 'baseline' })
    const experimental = prepareSearch({ query: 'orchid', ranking: 'experiment', inputs: '{"boost":3}' })
    const request = JSON.parse(prepareDiagnostics(index, original, 'source', { intent: 'experiment', experiment: experimental }).canonical)
    expect(request.request.ranking.profile).toBe('baseline')
    expect(request.experiment.ranking.profile).toBe('experiment')
  })
  it('redacts sensitive normalized and raw payloads unless deliberately included', () => {
    const query = prepareSearch({ query: 'PRIVATE_QUERY', inputs: '{"secret":42}' })
    const request = prepareDiagnostics(index, query, 'source', { intent: 'explain-hit' })
    const response: ClientDiagnostics = { execution: { id: 'rerun', kind: 'DiagnosticRerun', complete: true, requestHash: 'hash', target: 'products', generation: 'G1' },
      findings: [{ code: 'finding', message: 'PRIVATE_QUERY', evidenceIds: ['e'] }],
      evidence: [{ id: 'e', section: 'Query', status: 'Available', provenance: 'Engine', executionId: 'rerun', message: 'PRIVATE_QUERY', values: { secret: 'PRIVATE_VALUE' }, canonicalRequest: query.canonical }],
      hits: [], rawEngine: [{ engine: 'Vespa', version: 'pinned', executionId: 'rerun', section: 'Trace', status: 'Available', mediaType: 'text/plain', payload: 'PRIVATE_RAW' }], warnings: [], canonicalResponse: '{}' }
    const safe = diagnosticBundle(response, request, { query: false, values: false, raw: false })
    expect(safe).not.toContain('PRIVATE_')
    expect(safe).not.toContain('secret')
    const full = diagnosticBundle(response, request, { query: true, values: true, raw: true })
    for (const value of ['PRIVATE_QUERY', 'PRIVATE_VALUE', 'PRIVATE_RAW']) expect(full).toContain(value)
  })
  it('only compares aligned numeric signals and does not assign importance across scales', () => {
    expect(signalComparison(4, 3)).toBe('Numerically favours A')
    expect(signalComparison(3, 4)).toBe('Numerically favours B')
    expect(signalComparison(4, 4)).toBe('Tie')
    expect(signalComparison({ $type: 'integer', decimal: '9007199254740993' }, 4)).toContain('unavailable')
  })
})
