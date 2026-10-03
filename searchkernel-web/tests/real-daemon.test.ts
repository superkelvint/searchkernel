import { test, expect } from 'vitest'
import { chromium, type Browser } from 'playwright'
import { spawn, type ChildProcess } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { createServer, createConnection } from 'node:net'
import { Daemon, Index, Text, Integer, IndexDefinitionT } from '@searchkernel/client/node'

async function port() {
  const server = createServer()
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const value = (server.address() as { port: number }).port
  await new Promise<void>(done => server.close(() => done()))
  return value
}
async function stop(process: ChildProcess | undefined) {
  if (!process || process.exitCode !== null) return
  const exited = once(process, 'exit')
  process.kill('SIGTERM')
  await exited
}
async function reachable(endpoint: string, process: ChildProcess) {
  for (let attempt = 0; attempt < 200; attempt++) {
    if (process.exitCode !== null) throw new Error(`process exited: ${process.exitCode}`)
    try {
      const address = new URL(endpoint)
      await new Promise<void>((done, reject) => {
        const socket = createConnection({ host: address.hostname, port: Number(address.port) })
        socket.once('connect', () => { socket.destroy(); done() })
        socket.once('error', reject)
      })
      return
    } catch { await new Promise(done => setTimeout(done, 50)) }
  }
  throw new Error(`listener did not start: ${endpoint}`)
}

test.skipIf(process.env.SEARCHKERNEL_FRONTEND_REAL_ENGINE !== '1')('real browser discovery/open/search/documents survive restart and reject reincarnation', async () => {
  const root = await mkdtemp(resolve(tmpdir(), 'searchkernel-web-e2e-'))
  const grpcPort = await port(), webPort = await port(), uiPort = await port()
  const endpoint = `127.0.0.1:${grpcPort}`, webEndpoint = `http://127.0.0.1:${webPort}`
  let daemon: ChildProcess | undefined, vite: ChildProcess | undefined, browser: Browser | undefined
  const handles: Index[] = []
  const definition = () => new IndexDefinitionT('document_schema', [
    new Text({ stored: true, searchable: true }).definition('title'),
    new Integer({ stored: true }).definition('quantity'),
  ])
  async function create(target: string) {
    const index = await Index.create(endpoint, target, definition())
    handles.push(index)
    return index
  }
  async function start() {
    daemon = spawn(resolve(process.env.CARGO_TARGET_DIR ?? '../target/host', 'debug/searchkerneld'),
      ['--data-root', root, '--listen', `127.0.0.1:${grpcPort}`, '--web-listen', `127.0.0.1:${webPort}`, '--web-origin', `http://127.0.0.1:${uiPort}`], { stdio: 'inherit' })
    await reachable(webEndpoint, daemon)
  }
  try {
    await start()
    const alpha = await create('alpha')
    const beta = await create('beta')
    await alpha.upsert('one', { title: 'orchid blue', quantity: 7 })
    await alpha.upsert('two', { title: 'tulip red', quantity: 11 })
    await beta.upsert('sibling', { title: 'orchid sibling', quantity: 42 })
    await Promise.all(handles.map(index => index.close()))
    await stop(daemon)
    await start() // Neither durable target has been opened in this incarnation.
    const nodeDaemon = new Daemon(endpoint)
    try { expect(await nodeDaemon.listIndexes()).toEqual(['alpha', 'beta']) }
    finally { await nodeDaemon.close() }
    vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', String(uiPort)], {
      env: { ...process.env, VITE_SEARCHKERNEL_ENDPOINT: webEndpoint }, stdio: 'inherit',
    })
    await reachable(`http://127.0.0.1:${uiPort}`, vite)
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] })
    const page = await browser.newPage()
    page.on('pageerror', error => console.error('Browser page error:', error))
    page.on('console', message => {
      if (message.type() === 'error') console.error('Browser console:', message.text())
    })
    page.on('requestfailed', request => console.error('Browser request failed:', request.url(), request.failure()))
    await page.goto(`http://127.0.0.1:${uiPort}/indexes`)
    try { await page.getByRole('button', { name: 'alpha', exact: true }).waitFor() }
    catch (error) { console.error('Index picker page:', await page.locator('body').innerText()); throw error }
    await page.getByRole('button', { name: 'beta', exact: true }).waitFor()
    await page.getByRole('button', { name: 'alpha', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor()
    await page.getByRole('button', { name: 'Search documents', exact: true }).click()
    await page.getByRole('textbox', { name: 'Search this index' }).fill('orchid')
    await page.locator('form').getByRole('button').click()
    await page.getByText(/^1 matches/).waitFor()
    await page.locator('article').filter({ hasText: 'orchid blue' }).waitFor()
    expect(await page.locator('article').count()).toBe(1)
    await page.goto(`http://127.0.0.1:${uiPort}/indexes/alpha/data/documents`)
    await page.getByRole('button', { name: 'Open one', exact: true }).waitFor()
    await page.getByRole('button', { name: 'Open two', exact: true }).waitFor()
    await page.getByRole('button', { name: 'Open one', exact: true }).click()
    await page.getByRole('dialog').getByText('orchid blue', { exact: true }).waitFor()
    await page.goto(`http://127.0.0.1:${uiPort}/indexes/alpha/data/documents/missing`)
    await page.getByRole('dialog').getByText('The requested document was not found.', { exact: true }).waitFor()

    // Load a native browser module; Vitest transforms imports inside serialized
    // evaluate callbacks into SSR helpers that do not exist in Chromium.
    await page.addScriptTag({ type: 'module', content: `
      import { createDaemonSearchKernelClient } from '/src/api/daemon-searchkernel-client.ts';
      window.testAdapter = createDaemonSearchKernelClient(${JSON.stringify(webEndpoint)});
    ` })
    const observed = await page.evaluate(async () => {
      const adapter = (window as any).testAdapter
      return {
        indexes: await adapter.listIndexes(), metadata: await adapter.getIndex('alpha'),
        sibling: await adapter.browseDocuments('beta', { query: '', limit: 10 }),
        found: await adapter.getDocument('alpha', 'one'), missing: await adapter.getDocument('alpha', 'sibling'),
        search: await adapter.search('alpha', { query: 'orchid', limit: 10 }),
      }
    })
    expect(observed.indexes).toEqual([{ id: 'alpha' }, { id: 'beta' }])
    expect(observed.metadata.id).toBe('alpha')
    expect(observed.metadata.searchDefaults).toEqual({})
    expect(observed.sibling.documents.map((item: { id: string }) => item.id)).toEqual(['sibling'])
    expect(observed.found.fields.title).toBe('orchid blue')
    expect(observed.missing).toBeUndefined()
    expect(observed.search.matches).toBe('1')
    expect(observed.search.hits[0].document.id).toBe('one')
    expect(observed.search.resolved).toBeUndefined()

    await Index.delete(endpoint, 'alpha')
    const replacement = await create('alpha')
    await replacement.upsert('one', { title: 'replacement incarnation', quantity: 99 })
    const stale = await page.evaluate(async () => {
      try { await (window as any).testAdapter.getDocument('alpha', 'one'); return null }
      catch (error) { const e = error as any; return { code: e.code, retryable: e.retryable, details: e.details, origin: e.origin } }
    })
    expect(stale).toMatchObject({ code: 'failed_precondition', retryable: false, origin: 'canonical' })
    expect(JSON.stringify(stale?.details)).toContain('stale_index_handle')
    const sibling = await Index.open(endpoint, 'beta')
    handles.push(sibling)
    expect((await sibling.get('sibling'))?.title).toBe('orchid sibling')
  } finally {
    await browser?.close()
    await stop(vite)
    await Promise.all(handles.map(index => index.close()))
    await stop(daemon)
    await rm(root, { recursive: true, force: true })
  }
}, 120_000)
