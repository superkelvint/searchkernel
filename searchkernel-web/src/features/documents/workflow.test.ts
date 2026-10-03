import { describe, it, expect, vi } from 'vitest'
import { Tensor, toCanonicalJson, value } from '@searchkernel/client'
import { prepareDocumentMutation, preparedDocumentRequest, mutationOperation } from '../../api/document-request'
import { prepareSearch } from '../../api/search-request'
import { SearchKernelClientError, type DocumentEditorState, type SearchKernelClient } from '../../api/searchkernel-client'
import { decodeDocumentId, encodeDocumentId, documentPath, mutateAndVerify } from './workflow'
import { errorJson } from '../../components/Workbench'

const editor: DocumentEditorState = { operation: 'insert', id: 'doc/%2F café?#', fields: JSON.stringify({ title: 'orchid', active: false, amount: 2.5, absent: null, integer: { kind_type: 'Int64Value', kind: { value: '9007199254740993' } }, embedding: toCanonicalJson(value(Tensor.dense('tensor<float>(x[2])', [1, 2]))), people: [{ name: 'Kai', age: 32 }] }), operations: '[]' }
const browse = prepareSearch({ query: 'orchid', limit: 2 })

describe('canonical document workflow', () => {
  it('prepares create-only and atomic upsert without read/merge semantics and preserves typed fields', () => {
    for (const operation of ['insert', 'upsert'] as const) {
      const prepared = prepareDocumentMutation({ ...editor, operation }), request = JSON.parse(prepared.canonical)
      expect(request).toEqual(toCanonicalJson(preparedDocumentRequest(prepared)))
      expect(request.document.id).toBe(editor.id)
      const fields = Object.fromEntries(request.document.fields.map((f: any) => [f.key, f.value])) as any
      expect(fields.integer.kind.value).toBe('9007199254740993')
      expect(fields.active).toMatchObject({ kind_type: 'BoolValue', kind: { value: false } })
      expect(fields.absent.kind_type).toBe('NullValueValue')
      expect(fields.amount.kind_type).toBe('Float64Value')
      expect(fields.embedding).toEqual(toCanonicalJson(value(Tensor.dense('tensor<float>(x[2])', [1, 2]))))
      expect(fields.people.kind_type).toBe('ValueList')
    }
  })
  it('constructs patch set/unset and all executable update forms, including dense tensor assignment and create', () => {
    const kinds = ['assign', 'clear', 'increment', 'decrement', 'multiply', 'divide']
    for (const kind of kinds) {
      const operation = mutationOperation('update', 'amount', kind, kind === 'assign' ? JSON.stringify(toCanonicalJson(value(Tensor.dense('tensor<float>(x[2])', [3, 4])))) : '2')
      const request = JSON.parse(prepareDocumentMutation({ ...editor, operation: 'update', operations: JSON.stringify([operation]), create: true }).canonical)
      expect(request.operations[0].target.path).toBe('amount')
      expect(request.operations[0].operation_type).toBe(kind === 'assign' ? 'AssignUpdate' : kind === 'clear' ? 'ClearUpdate' : 'ArithmeticUpdate')
      if (!['assign', 'clear'].includes(kind)) expect(request.operations[0].operation.operator).toBe(kind[0].toUpperCase() + kind.slice(1))
      expect(request.create.value).toBe(true)
    }
    const operations = [mutationOperation('patch', 'title', 'assign', '"changed"'), mutationOperation('patch', 'amount', 'clear', '')]
    const request = JSON.parse(prepareDocumentMutation({ ...editor, operation: 'patch', operations: JSON.stringify(operations) }).canonical)
    expect(request.operations.map((op: any) => op.operation_type)).toEqual(['SetField', 'UnsetField'])
    expect(request.document).toBeUndefined()
    const nested = mutationOperation('update', 'people', 'assign', '[{"name":"Kai","active":false,"count":{"kind_type":"UInt64Value","kind":{"value":"18446744073709551615"}},"raw":{"kind_type":"BytesValue","kind":{"value":[0,255]}}}]') as any
    expect(nested.operation.value.kind_type).toBe('ValueList')
    const members = Object.fromEntries(nested.operation.value.kind.values[0].kind.fields.map((entry: any) => [entry.key, entry.value])) as any
    expect(members.count.kind.value).toBe('18446744073709551615')
    expect(members.raw.kind.value).toEqual([0, 255])
    expect(members.active.kind.value).toBe(false)
  })
  it('preserves deletion/conditions, rejects malformed shapes, and does not independently validate schema meaning', () => {
    expect(JSON.parse(prepareDocumentMutation({ ...editor, operation: 'delete', condition: 'document.title == "orchid"' }).canonical)).toEqual({ id: editor.id, condition: 'document.title == "orchid"' })
    for (const fields of ['{', '[]', 'null', '{"integer":{"kind_type":"Int64Value","kind":{"value":9007199254740992}}}']) expect(() => prepareDocumentMutation({ ...editor, fields })).toThrow()
    expect(() => prepareDocumentMutation({ ...editor, operation: 'update', operations: '[{"operation_type":"Bogus"}]' })).toThrow()
    expect(() => prepareDocumentMutation({ ...editor, fields: '{"unknown_field":true}' })).not.toThrow()
    expect(() => preparedDocumentRequest({ operation: 'delete', id: 'forged', canonical: '{}' })).toThrow()
  })
  it('round trips arbitrary literal IDs without percent/slash or Unicode reinterpretation', () => {
    for (const id of ['one', 'id:namespace::x', 'a/b', '%2F', '%252F', ' spaces ?#', 'café 日本 😀']) {
      const token = encodeDocumentId(id)
      expect(decodeDocumentId(token)).toBe(id)
      expect(documentPath('alpha', id)).toBe(`/indexes/alpha/data/documents/by-id/${token}`)
      expect(token).not.toContain('%')
    }
    for (const token of ['broken', 'u.', 'u.!!!', 'u._w', 'u.Zg=']) expect(() => decodeDocumentId(token)).toThrow()
  })
  it('keeps semantic details and transport status inspectable without leaking stacks', () => {
    const semantic = new SearchKernelClientError('unknown field', { origin: 'canonical', code: 'invalid_argument', retryable: false, details: [{ code: 'unknown_field', path: 'operations[0].target' }] })
    expect(JSON.parse(errorJson(semantic))).toMatchObject({ origin: 'canonical', code: 'invalid_argument', retryable: false, details: [{ code: 'unknown_field', path: 'operations[0].target' }] })
    expect(errorJson(semantic)).not.toContain('stack')
    expect(JSON.parse(errorJson(new SearchKernelClientError('offline', { origin: 'transport', code: 'transport_error', status: 503, retryable: true })))).toMatchObject({ origin: 'transport', status: 503, retryable: true })
  })
  it('reports completion only after acknowledged write and fresh GET/search, forwarding cancellation', async () => {
    const events: string[] = [], signal = new AbortController().signal
    const executeDocumentMutation = vi.fn(async (..._args: unknown[]) => { events.push('ack'); return { operation: 'insert' as const, id: editor.id, canonicalResponse: '{}' } })
    const getDocument = vi.fn(async () => { events.push('get'); return { id: editor.id, fields: { title: 'fresh' } } })
    const executeSearch = vi.fn(async () => { events.push('search'); return { request: {}, matches: '1', hits: [], diagnostics: [] } })
    const client = { executeDocumentMutation, getDocument, executeSearch } as unknown as SearchKernelClient
    const result = await mutateAndVerify(client, 'alpha', prepareDocumentMutation(editor), browse, { signal })
    expect(result.document?.fields.title).toBe('fresh'); expect(events).toEqual(['ack', 'get', 'search'])
    expect(result.writeOutcome).toBe('applied')
    expect(result.readbackPresence).toBe('present')
    expect(executeDocumentMutation.mock.calls[0]?.[2]).toBeUndefined()
    expect(getDocument).toHaveBeenCalledWith('alpha', editor.id, { signal })
    expect(executeSearch).toHaveBeenCalledWith('alpha', browse, { signal })
    executeDocumentMutation.mockRejectedValueOnce(new SearchKernelClientError('conflict', { code: 'already_exists', origin: 'canonical', details: { path: 'document.id' } }) as never)
    await expect(mutateAndVerify(client, 'alpha', prepareDocumentMutation(editor), browse)).rejects.toMatchObject({ code: 'already_exists', details: { path: 'document.id' } })
    expect(getDocument).toHaveBeenCalledTimes(1)
  })
  it('treats a lost mutation response as an unknown outcome instead of a retryable failure', async () => {
    const executeDocumentMutation = vi.fn().mockRejectedValue(new SearchKernelClientError('connection lost', { code: 'transport_error', origin: 'transport', retryable: true }))
    const getDocument = vi.fn(), executeSearch = vi.fn()
    const client = { executeDocumentMutation, getDocument, executeSearch } as unknown as SearchKernelClient
    await expect(mutateAndVerify(client, 'alpha', prepareDocumentMutation({ ...editor, operation: 'update', operations: JSON.stringify([mutationOperation('update', 'amount', 'increment', '2')]) }), browse)).rejects.toMatchObject({
      code: 'mutation_outcome_unknown',
      retryable: false,
      details: { outcome: 'unknown', operation: 'update', id: editor.id, cause: { code: 'transport_error', retryable: true } },
    })
    expect(getDocument).not.toHaveBeenCalled()
    expect(executeSearch).not.toHaveBeenCalled()
  })
  it('distinguishes a committed mutation with failed reads and catches delete/write visibility violations', async () => {
    const client = { executeDocumentMutation: vi.fn().mockResolvedValue({ operation: 'delete', id: editor.id, canonicalResponse: '{"deleted":true}', deleted: true }), getDocument: vi.fn().mockResolvedValue({ id: editor.id, fields: {} }), executeSearch: vi.fn().mockResolvedValue({}) } as unknown as SearchKernelClient
    await expect(mutateAndVerify(client, 'alpha', prepareDocumentMutation({ ...editor, operation: 'delete' }), browse)).rejects.toMatchObject({ code: 'visibility_error', retryable: false, details: { acknowledged: true } })
    vi.mocked(client.getDocument).mockResolvedValue(undefined)
    await expect(mutateAndVerify(client, 'alpha', prepareDocumentMutation(editor), browse)).rejects.toMatchObject({ code: 'visibility_error', details: { acknowledged: true } })
    vi.mocked(client.executeDocumentMutation).mockResolvedValue({ operation: 'delete', id: editor.id, canonicalResponse: '{"deleted":false}', deleted: false })
    const missing = await mutateAndVerify(client, 'alpha', prepareDocumentMutation({ ...editor, operation: 'delete' }), browse)
    expect(missing.writeOutcome).toBe('not_deleted')
    expect(missing.readbackPresence).toBe('absent')
    vi.mocked(client.getDocument).mockRejectedValue(new SearchKernelClientError('offline', { code: 'transport_error', origin: 'transport', status: 503, retryable: true }))
    await expect(mutateAndVerify(client, 'alpha', prepareDocumentMutation(editor), browse)).rejects.toMatchObject({ code: 'transport_error', origin: 'transport', status: 503, retryable: false, details: { acknowledged: true, cause: { retryable: true } } })
  })
})
