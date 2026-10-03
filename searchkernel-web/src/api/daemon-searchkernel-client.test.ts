import { describe, expect, it, vi, afterEach } from 'vitest'
import { Daemon, Index as RemoteIndex, IndexDefinitionT, Text, SearchKernelError, TransportError } from '@searchkernel/client'
import { createDaemonSearchKernelClient, dashboardValue, daemonError } from './daemon-searchkernel-client'

afterEach(() => vi.restoreAllMocks())
describe('daemon dashboard boundary', () => {
  it('lists without open, hydrates only the selected target, and retains its bound handle', async () => {
    vi.spyOn(Daemon.prototype, 'listIndexes').mockResolvedValue(['alpha', 'beta'])
    const get = vi.fn().mockResolvedValue({ _id: 'one', title: 'real title', quantity: 9007199254740993n })
    const definition = new IndexDefinitionT('different-schema', [new Text({ stored: true, searchable: true }).definition('title')])
    const open = vi.spyOn(RemoteIndex, 'open').mockResolvedValue({ definition, generation: 'incarnation', get } as unknown as RemoteIndex)
    const client = createDaemonSearchKernelClient('http://daemon')
    expect(await client.listIndexes()).toEqual([{ id: 'alpha' }, { id: 'beta' }])
    expect(open).not.toHaveBeenCalled()
    expect(await client.getIndex('alpha')).toMatchObject({ id: 'alpha', generation: 'incarnation', searchDefaults: {}, profiles: [], summaries: [] })
    expect(await client.getIndex('alpha')).toMatchObject({ id: 'alpha', generation: 'incarnation' })
    expect((await client.getDocument('alpha', 'one'))?.fields.quantity).toEqual({ $type: 'integer', decimal: '9007199254740993' })
    expect(open).toHaveBeenCalledTimes(1)
    get.mockResolvedValue(null)
    expect(await client.getDocument('alpha', 'missing')).toBeUndefined()
  })
  it('does not let one cancelled metadata observer abort the shared index open', async () => {
    let resolveOpen!: (index: RemoteIndex) => void
    const openPromise = new Promise<RemoteIndex>(resolve => { resolveOpen = resolve })
    const open = vi.spyOn(RemoteIndex, 'open').mockReturnValue(openPromise)
    const client = createDaemonSearchKernelClient('http://daemon')
    const first = new AbortController()
    const cancelled = client.getIndex('alpha', { signal: first.signal })
    first.abort()
    await expect(cancelled).rejects.toMatchObject({ code: 'aborted' })

    const replacement = client.getIndex('alpha')
    resolveOpen({
      definition: new IndexDefinitionT('schema'),
      generation: 'incarnation',
    } as unknown as RemoteIndex)
    await expect(replacement).resolves.toMatchObject({ id: 'alpha', generation: 'incarnation' })
    expect(open).toHaveBeenCalledTimes(1)
    expect(open.mock.calls[0]?.[3]).toBeUndefined()
  })

  it('preserves disabled attributes and leaves script-owned capabilities unavailable', async () => {
    const disabled = new Text({ stored: true, searchable: true }).definition('title')
    Object.assign(disabled, { attribute: { enabled: { value: false } } })
    const scripted = new Text().definition('scripted')
    scripted.indexingScript = 'input scripted | index | summary'
    vi.spyOn(RemoteIndex, 'open').mockResolvedValue({
      definition: new IndexDefinitionT('schema', [disabled, scripted]), generation: 'incarnation',
    } as unknown as RemoteIndex)
    const index = await createDaemonSearchKernelClient('http://daemon').getIndex('alpha')
    expect(index?.fields[0].attribute).toBe(false)
    expect(index?.fields[1]).toMatchObject({ stored: undefined, searchable: undefined, attribute: undefined })
  })
  it('passes explicit controls and cancellation and leaves omitted defaults absent', async () => {
    const execute = vi.fn().mockResolvedValue({ totalHits: 1n, hits: [{ id: 'one', fields: { title: 'orchid' }, score: 0.75 }], issues: [], coverage: null, timing: null })
    const builder = { limit: vi.fn(), ranking: vi.fn(), summary: vi.fn(), execute }
    const search = vi.fn(() => builder), browse = vi.fn(() => builder)
    vi.spyOn(RemoteIndex, 'open').mockResolvedValue({ definition: new IndexDefinitionT(), search, browse } as unknown as RemoteIndex)
    const client = createDaemonSearchKernelClient('http://daemon')
    const signal = new AbortController().signal
    const result = await client.search('alpha', { query: 'orchid', ranking: 'custom', summary: 'compact', limit: 3 }, { signal })
    expect(builder.ranking).toHaveBeenCalledWith('custom')
    expect(builder.summary).toHaveBeenCalledWith('compact')
    expect(builder.limit).toHaveBeenCalledWith(3)
    expect(execute).toHaveBeenCalledWith({ signal })
    expect(result).toMatchObject({ matches: '1', hits: [{ document: { id: 'one' }, score: '0.75' }] })
    expect(result.resolved).toBeUndefined()
    expect(result.analysis).toBeUndefined()
    expect(result.timingMs).toBeUndefined()
    vi.clearAllMocks()
    await client.search('alpha', { query: '' })
    expect(browse).toHaveBeenCalledOnce()
    expect(builder.ranking).not.toHaveBeenCalled()
    expect(builder.summary).not.toHaveBeenCalled()
    expect(builder.limit).not.toHaveBeenCalled()
  })
  it('keeps stale errors structured and never silently reopens', async () => {
    const error = Object.assign(new SearchKernelError('stale'), { error: { code: 5, message: 'stale', retryable: false, details: [{ code: 'stale_index_handle', metadata: [] }] } })
    const get = vi.fn().mockRejectedValue(error)
    const open = vi.spyOn(RemoteIndex, 'open').mockResolvedValue({ definition: new IndexDefinitionT(), get } as unknown as RemoteIndex)
    const client = createDaemonSearchKernelClient('http://daemon')
    await client.getIndex('alpha')
    await expect(client.getDocument('alpha', 'one')).rejects.toMatchObject({ origin: 'canonical', retryable: false, details: [{ code: 'stale_index_handle' }] })
    await expect(client.getDocument('alpha', 'one')).rejects.toMatchObject({ origin: 'canonical' })
    expect(open).toHaveBeenCalledOnce()
    expect(daemonError(new TransportError('offline'))).toMatchObject({ origin: 'transport', retryable: true })
    expect(daemonError(new TransportError('cancelled', undefined, 'aborted'))).toMatchObject({ code: 'aborted', retryable: false })
  })
  it('represents richer values without integer truncation or JSON failure', () => {
    const value = dashboardValue({ dense: [1, 2], integer: 9007199254740993n, bytes: new Uint8Array([0, 255]) })
    expect(JSON.stringify(value)).toContain('9007199254740993')
    expect(value).toMatchObject({ bytes: { $type: 'bytes', base64: 'AP8=' } })
    expect(() => dashboardValue(Symbol('unsupported'))).toThrow('Unsupported result value')
  })
})
