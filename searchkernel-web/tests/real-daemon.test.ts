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

test.skipIf(process.env.SEARCHKERNEL_FRONTEND_REAL_ENGINE !== '1')('W2 real document mutations are immediately visible and persist through browser and daemon reopen', async () => {
  const { Float, Boolean: BoolField, ArrayOfStructField, TensorField, Tensor, value, toCanonicalJson } = await import('@searchkernel/client/node')
  const root = await mkdtemp(resolve(tmpdir(), 'searchkernel-w2-e2e-'))
  const grpcPort = await port(), webPort = await port(), uiPort = await port()
  const endpoint = `127.0.0.1:${grpcPort}`, webEndpoint = `http://127.0.0.1:${webPort}`, ui = `http://127.0.0.1:${uiPort}`
  let daemon: ChildProcess | undefined, vite: ChildProcess | undefined, browser: Browser | undefined, index: Index | undefined
  const id = 'w2/%2F café?#', tensor = (cells: number[]) => toCanonicalJson(value(Tensor.dense('tensor<float>(x[2])', cells)))
  const fields = (title: string, quantity: number) => ({ title, quantity, amount: 2.5, active: false, note: 'preserve me', embedding: tensor([1, 2]), people: [{ name: 'Kai', age: 32 }] })
  async function start() {
    daemon = spawn(resolve(process.env.CARGO_TARGET_DIR ?? '../target/host', 'debug/searchkerneld'), ['--data-root', root, '--listen', `127.0.0.1:${grpcPort}`, '--web-listen', `127.0.0.1:${webPort}`, '--web-origin', ui], { stdio: 'inherit' })
    await reachable(webEndpoint, daemon)
  }
  try {
    await start()
    index = await Index.create(endpoint, 'documents', new IndexDefinitionT('document_workflow', [
      new Text({ stored: true, searchable: true }).definition('title'),
      new Integer({ stored: true, attribute: true }).definition('quantity'),
      new Float({ stored: true }).definition('amount'), new BoolField({ stored: true }).definition('active'),
      new Text({ stored: true }).definition('note'), new TensorField('tensor<float>(x[2])', { stored: true }).definition('embedding'),
      new ArrayOfStructField({ name: new Text(), age: new Integer() }, { stored: true }).definition('people'),
    ]))
    vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', String(uiPort)], { env: { ...process.env, VITE_SEARCHKERNEL_ENDPOINT: webEndpoint }, stdio: 'inherit' })
    await reachable(ui, vite)
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] })
    const page = await browser.newPage(), errors: string[] = []
    page.on('pageerror', error => errors.push(error.message))
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write'], { origin: ui })
    await page.goto(`${ui}/indexes`)
    await page.getByRole('button', { name: 'documents', exact: true }).click()
    await page.goto(`${ui}/indexes/documents/data/documents`)
    try { await page.getByText('No matching documents. Create a document or change the search.', { exact: true }).waitFor() }
    catch (error) { console.error('W2 empty index page:', await page.locator('body').innerText()); throw error }
    await page.getByRole('button', { name: 'Create document', exact: true }).click()
    await page.getByLabel('Mutation document ID').fill(id)
    await page.getByLabel('Document fields', { exact: true }).fill(JSON.stringify(fields('orchid seed', 8)))
    await page.getByText('Mutation request', { exact: true }).click()
    const request = JSON.parse(await page.getByLabel('Mutation request inspector').innerText())
    expect(request.document.id).toBe(id)
    expect(request.document.fields.find((f: any) => f.key === 'active').value).toMatchObject({ kind_type: 'BoolValue', kind: { value: false } })
    await page.getByRole('button', { name: 'Copy Mutation request', exact: true }).click()
    expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(request)
    await page.route(`${webEndpoint}/**`, async route => { if (route.request().method() === 'POST') await new Promise(done => setTimeout(done, 200)); await route.continue() })
    const submit = () => page.getByRole('button', { name: 'Submit document mutation', exact: true }).click()
    const acknowledged = async (operation: string) => {
      const presence = operation === 'delete' ? 'absent' : 'present'
      try { await page.getByText(`${operation} acknowledged. Fresh backend GET/search completed; GET confirms document ${presence}.`, { exact: true }).waitFor() }
      catch (error) { console.error(`W2 ${operation} page:`, await page.locator('body').innerText()); throw error }
    }
    await submit()
    await page.getByText('Mutation is running; after acknowledgement, fresh backend GET/search reads will run.', { exact: true }).waitFor()
    expect(await page.getByRole('button', { name: 'Running insert…', exact: true }).isDisabled()).toBe(true)
    await acknowledged('insert')
    await page.unroute(`${webEndpoint}/**`)
    await page.getByText('Read-back GET', { exact: true }).click()
    expect(await page.getByLabel('Read-back GET inspector').innerText()).toContain('orchid seed')
    await page.getByText('Read-back search', { exact: true }).click()
    expect(await page.getByLabel('Read-back search inspector').innerText()).toContain('orchid seed')
    expect((await index.get(id))?.quantity).toBe(8n)
    expect((await index.search('orchid').execute()).hits.map(hit => hit.id)).toEqual([id])
    await submit() // Insert is create-only, never an accidental upsert.
    await page.getByRole('alert').filter({ hasText: 'canonical' }).waitFor()
    expect(await page.getByRole('alert').innerText()).toContain('already_exists')
    expect(await page.getByText('insert acknowledged.', { exact: false }).count()).toBe(0)
    await page.getByRole('button', { name: `Open ${id}`, exact: true }).click()
    await page.getByRole('dialog').getByText('orchid seed', { exact: true }).waitFor()
    const detailUrl = page.url()
    expect(detailUrl).toContain('/documents/by-id/u.')
    await page.getByRole('button', { name: 'Copy detail URL', exact: true }).click()
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(detailUrl)
    await page.getByLabel('Document operation', { exact: true }).selectOption('upsert')
    await page.getByLabel('Document fields', { exact: true }).fill(JSON.stringify(fields('orchid replaced', 8)))
    await submit(); await acknowledged('upsert')
    expect((await index.get(id))?.title).toBe('orchid replaced')
    expect((await index.search('replaced').execute()).hits.map(hit => hit.id)).toEqual([id])
    await page.reload()
    await page.getByRole('dialog').getByText('orchid replaced', { exact: true }).waitFor()
    // A new browser session and a new daemon incarnation must read durable state,
    // without depending on a held response or the adapter's old generation.
    async function reopen() {
      await page.goto('about:blank')
      await index!.close(); await stop(daemon); await start()
      index = await Index.open(endpoint, 'documents')
      await page.goto(detailUrl)
    }
    await reopen()
    await page.getByRole('dialog').getByText('orchid replaced', { exact: true }).waitFor()
    const fresh = await browser.newPage()
    await fresh.goto(detailUrl)
    await fresh.getByRole('dialog').getByText('orchid replaced', { exact: true }).waitFor()
    await fresh.close()
    // Canonical patch sets and unsets without replacing other fields.
    await page.getByLabel('Document operation', { exact: true }).selectOption('patch')
    await page.getByLabel('Mutation field').selectOption('title')
    await page.getByLabel('Update value').fill('"orchid patched"')
    await page.getByRole('button', { name: 'Add operation', exact: true }).click()
    await page.getByLabel('Mutation field').selectOption('note')
    await page.getByLabel('Update kind').selectOption('clear')
    await page.getByRole('button', { name: 'Add operation', exact: true }).click()
    await submit(); await acknowledged('patch')
    expect(await index.get(id)).toMatchObject({ title: 'orchid patched', quantity: 8n, active: false, amount: 2.5, people: [{ name: 'Kai', age: 32n }] })
    expect((await index.get(id))?.note).toBeUndefined()
    expect((await index.search('patched').execute()).hits[0].fields.quantity).toBe(8n)
    await page.getByLabel('Document operation', { exact: true }).selectOption('update')
    // Exercise every currently executable update family through the real UI,
    // checking a fresh numeric filter as well as GET after each arithmetic write.
    for (const [kind, operand, expected] of [['increment', 5, 13], ['decrement', 3, 10], ['multiply', 2, 20], ['divide', 4, 5]] as const) {
      await page.getByLabel('Document operations').fill('[]')
      await page.getByLabel('Mutation field').selectOption('quantity')
      await page.getByLabel('Update kind').selectOption(kind)
      await page.getByLabel('Update value').fill(String(operand))
      await page.getByRole('button', { name: 'Add operation', exact: true }).click()
      await submit(); await acknowledged('update')
      expect((await index.get(id))?.quantity).toBe(BigInt(expected))
      expect((await index.search('patched').filterField('quantity', 'eq', expected).execute()).hits.map(hit => hit.id)).toEqual([id])
    }
    await page.getByLabel('Document operations').fill('[]')
    await page.getByLabel('Mutation field').selectOption('embedding')
    await page.getByLabel('Update kind').selectOption('assign')
    await page.getByLabel('Update value').fill(JSON.stringify(tensor([3, 4])))
    await page.getByRole('button', { name: 'Add operation', exact: true }).click()
    await page.getByLabel('Mutation field').selectOption('amount')
    await page.getByLabel('Update kind').selectOption('clear')
    await page.getByRole('button', { name: 'Add operation', exact: true }).click()
    await submit(); await acknowledged('update')
    const updated = await index.get(id)
    expect(updated?.embedding).toMatchObject({ $searchkernel_tensor: { typeSpec: 'tensor<float>(x[2])', dense: [3, 4] } })
    expect(updated?.amount).toBeUndefined()
    // Native default summaries deliberately omit tensors/array-of-structs.
    // GET above verifies the complete typed value; ordinary search fields must
    // still observe the updated document through the canonical projection.
    expect((await index.search('patched').filterField('quantity', 'eq', 5).execute()).hits.map(hit => hit.id)).toEqual([id])
    await reopen()
    await page.getByRole('dialog').getByText('orchid patched', { exact: true }).waitFor()
    expect(await index.get(id)).toMatchObject({ quantity: 5n, embedding: { $searchkernel_tensor: { dense: [3, 4] } } })
    expect((await index.get(id))?.amount).toBeUndefined()
    // Malformed editor input, portable validation, and transport failure are
    // separate states. None is reported as a successful mutation.
    await page.getByLabel('Document operations').fill('{broken')
    await page.getByRole('alert').filter({ hasText: 'client' }).waitFor()
    expect(await page.getByRole('button', { name: 'Submit document mutation', exact: true }).isDisabled()).toBe(true)
    await page.getByLabel('Document operations').fill(JSON.stringify([{ target: { path: 'unknown_field' }, operation_type: 'ClearUpdate', operation: {} }]))
    await submit()
    await page.getByRole('alert').filter({ hasText: 'canonical' }).waitFor()
    await page.getByText('Error', { exact: true }).click()
    expect(JSON.parse(await page.getByLabel('Error inspector').innerText())).toMatchObject({ origin: 'canonical', retryable: false })
    await page.getByLabel('Document operations').fill(JSON.stringify([{ target: { path: 'quantity' }, operation_type: 'AssignUpdate', operation: { value: toCanonicalJson(value(99)) } }]))
    await page.route(`${webEndpoint}/**`, route => route.request().method() === 'POST' ? route.abort('failed') : route.continue())
    await submit()
    await page.getByRole('alert').filter({ hasText: 'mutation_outcome_unknown' }).waitFor()
    expect(await page.getByRole('button', { name: 'Submit document mutation', exact: true }).isDisabled()).toBe(true)
    await page.unroute(`${webEndpoint}/**`)
    expect((await index.get(id))?.quantity).toBe(5n)
    await page.getByRole('button', { name: 'Read current document before retry', exact: true }).click()
    await page.getByText('Authoritative GET completed. Review the current document state before deciding whether to submit another mutation.', { exact: true }).waitFor()
    expect(await page.getByRole('button', { name: 'Submit document mutation', exact: true }).isDisabled()).toBe(false)
    // Update-create and upsert-create are server operations, not GET + PUT.
    await page.getByLabel('Mutation document ID').fill('created-by-update')
    await page.getByLabel('Document operations').fill(JSON.stringify([{ target: { path: 'title' }, operation_type: 'AssignUpdate', operation: { value: toCanonicalJson(value('orchid sibling')) } }]))
    await page.getByLabel('Create missing document').check()
    await submit(); await acknowledged('update')
    expect((await index.get('created-by-update'))?.title).toBe('orchid sibling')
    await page.getByLabel('Document operation', { exact: true }).selectOption('upsert')
    await page.getByLabel('Mutation document ID').fill('created-by-upsert')
    await page.getByLabel('Document fields', { exact: true }).fill(JSON.stringify(fields('orchid other', 42)))
    await submit(); await acknowledged('upsert')
    expect((await index.get('created-by-upsert'))?.quantity).toBe(42n)
    // Reopen the original route to avoid retaining another mutation's editor ID.
    await page.goto(detailUrl)
    await page.getByRole('dialog').getByText('orchid patched', { exact: true }).waitFor()
    await page.getByLabel('Document operation', { exact: true }).selectOption('delete')
    expect(await page.getByRole('button', { name: 'Submit document mutation', exact: true }).isDisabled()).toBe(true)
    await page.getByLabel('Confirm document deletion').check()
    await submit(); await acknowledged('delete')
    await page.getByRole('dialog').getByText('The requested document was not found.', { exact: true }).waitFor()
    expect(await index.get(id)).toBeNull() // SDK absence; dashboard seam maps it to undefined.
    expect((await index.search('patched').execute()).hits).toEqual([])
    await page.reload()
    await page.getByRole('dialog').getByText('The requested document was not found.', { exact: true }).waitFor()
    await reopen()
    await page.getByRole('dialog').getByText('The requested document was not found.', { exact: true }).waitFor()
    expect(await index.get(id)).toBeNull()
    expect((await index.search('patched').execute()).hits).toEqual([])
    expect((await index.get('created-by-upsert'))?.title).toBe('orchid other')
    expect((await index.browse().execute()).hits.map(hit => hit.id).sort()).toEqual(['created-by-update', 'created-by-upsert'])
    await page.getByRole('button', { name: 'Close document', exact: true }).click()
    await page.waitForURL(url => url.pathname === '/indexes/documents/data/documents')
    await expect.poll(() => page.getByRole('dialog').count()).toBe(0)
    await page.getByLabel('Document page size').fill('1')
    await expect.poll(() => JSON.parse(new URL(page.url()).searchParams.get('s') ?? '{}').state?.limit).toBe(1)
    try { await page.getByRole('button', { name: 'Next documents', exact: true }).click() }
    catch (error) { console.error('W2 pagination page:', page.url(), await page.locator('body').innerText()); throw error }
    await page.getByText(/offset 1/).waitFor()
    const listUrl = page.url()
    await page.reload()
    expect(page.url()).toBe(listUrl)
    await page.getByText(/offset 1/).waitFor()
    await page.getByLabel('Search documents', { exact: true }).fill('other')
    await page.getByRole('button', { name: 'Open created-by-upsert', exact: true }).waitFor()
    await page.getByText(/offset 0/).waitFor()
    expect(errors).toEqual([])
  } finally {
    await browser?.close(); await stop(vite); await index?.close(); await stop(daemon); await rm(root, { recursive: true, force: true })
  }
}, 180_000)

test.skipIf(process.env.SEARCHKERNEL_FRONTEND_REAL_ENGINE !== '1')('W1 real Search Lab constructs filters, sorting, pagination, typed inputs and reproducible inspectors', async () => {
  const { RankProfile } = await import('@searchkernel/client/node')
  const root = await mkdtemp(resolve(tmpdir(), 'searchkernel-w1-e2e-'))
  const grpcPort = await port(), webPort = await port(), uiPort = await port()
  const endpoint = `127.0.0.1:${grpcPort}`, webEndpoint = `http://127.0.0.1:${webPort}`
  const ui = `http://127.0.0.1:${uiPort}`
  let daemon: ChildProcess | undefined, vite: ChildProcess | undefined, browser: Browser | undefined, index: Index | undefined
  try {
    daemon = spawn(resolve(process.env.CARGO_TARGET_DIR ?? '../target/host', 'debug/searchkerneld'), ['--data-root', root, '--listen', `127.0.0.1:${grpcPort}`, '--web-listen', `127.0.0.1:${webPort}`, '--web-origin', ui], { stdio: 'inherit' })
    await reachable(webEndpoint, daemon)
    const definition = new IndexDefinitionT('workbench_schema', [
      new Text({ stored: true, searchable: true }).definition('title'),
      new Integer({ stored: true, attribute: true }).definition('quantity'),
    ])
    definition.rankProfiles = [new RankProfile('boosted').firstPhase('query(boost) * attribute(quantity)').matchFeature('query(boost)').definition()]
    index = await Index.create(endpoint, 'workbench', definition)
    await index.upsert('a', { title: 'orchid blue', quantity: 7 })
    await index.upsert('b', { title: 'orchid red', quantity: 11 })
    await index.upsert('c', { title: 'tulip white', quantity: 13 })
    await index.upsert('d', { title: 'orchid low', quantity: 3 })
    vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', String(uiPort)], { env: { ...process.env, VITE_SEARCHKERNEL_ENDPOINT: webEndpoint }, stdio: 'inherit' })
    await reachable(ui, vite)
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] })
    const page = await browser.newPage()
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write'], { origin: ui })
    const errors: string[] = []
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${ui}/indexes`)
    try { await page.getByRole('button', { name: 'workbench', exact: true }).waitFor() }
    catch (error) { console.error('W1 index picker:', await page.locator('body').innerText()); throw error }
    await page.getByRole('button', { name: 'workbench', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).click()
    await page.getByText('No query executed.', { exact: false }).waitFor()
    await page.getByLabel('Search this index').fill('orchid')
    // Delay actual browser requests to prove loading and duplicate-submit UX.
    await page.route(`${webEndpoint}/**`, async route => {
      if (route.request().method() === 'POST') await new Promise(done => setTimeout(done, 250))
      await route.continue()
    })
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.getByText('Running request… Previous results are retained.', { exact: true }).waitFor()
    expect(await page.locator('form').getByRole('button', { name: 'Running…' }).isDisabled()).toBe(true)
    await page.getByText(/^3 matches/).waitFor()
    await page.unroute(`${webEndpoint}/**`)
    await page.getByText('Advanced controls', { exact: true }).click()
    await page.getByRole('button', { name: 'Add filter', exact: true }).click()
    await page.getByLabel('Filter 1 field').selectOption('quantity')
    await page.getByLabel('Filter 1 operator').selectOption('gte')
    await page.getByLabel('Filter 1 value').fill('7')
    await page.getByRole('button', { name: 'Add filter', exact: true }).click()
    await page.getByLabel('Filter 2 field').selectOption('quantity')
    await page.getByLabel('Filter 2 operator').selectOption('lt')
    await page.getByLabel('Filter 2 value').fill('20')
    await page.getByRole('button', { name: 'Add sort', exact: true }).click()
    await page.getByLabel('Sort 1 field').selectOption('quantity')
    await page.getByLabel('Sort 1 direction').selectOption('asc')
    await page.getByLabel('Limit', { exact: true }).fill('1')
    await page.getByLabel('Ranking profile', { exact: true }).fill('boosted')
    await page.getByLabel('Query inputs', { exact: true }).fill('{"boost":2.5}')
    await expect.poll(async () => JSON.parse(await page.getByLabel('Request inspector', { exact: true }).innerText())).toMatchObject({
      ranking: { profile: 'boosted' }, inputs: [{ name: 'boost' }],
      filters: { structured: [{}, {}] }, sort: [{ direction: 'Asc' }], limit: { value: 1 },
    })
    const request = JSON.parse(await page.getByLabel('Request inspector', { exact: true }).innerText())
    expect(request.ranking.profile).toBe('boosted')
    expect(request.inputs[0].name).toBe('boost')
    expect(request.filters.structured).toHaveLength(2)
    expect(request.sort[0].direction).toBe('Asc')
    expect(request.limit.value).toBe(1)
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.locator('article').filter({ hasText: 'orchid blue' }).waitFor()
    // The previous successful response stays visible while the new request runs.
    await expect.poll(() => page.locator('article').count()).toBe(1)
    expect(await page.locator('article').innerText()).toContain('17.5')
    await page.getByRole('tab', { name: 'Response', exact: true }).click()
    const raw = JSON.parse(await page.getByLabel('Response inspector', { exact: true }).innerText())
    await page.getByRole('button', { name: 'Copy Response', exact: true }).click()
    expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(raw)
    expect(raw.total_hits.value).toBe('2')
    expect(JSON.stringify(raw)).toContain('query(boost)')
    await page.getByRole('button', { name: 'Next page', exact: true }).click()
    await page.locator('article').filter({ hasText: 'orchid red' }).waitFor()
    expect(await page.locator('article').count()).toBe(1)
    await page.getByRole('tab', { name: 'Request', exact: true }).click()
    expect(JSON.parse(await page.getByLabel('Request inspector').innerText()).offset.value).toBe(1)
    const sharedUrl = page.url()
    await page.getByRole('button', { name: 'Copy URL', exact: true }).click()
    const copiedUrl = await page.evaluate(() => navigator.clipboard.readText())
    expect(copiedUrl).toBe(sharedUrl)
    const fresh = await browser.newPage()
    await fresh.goto(copiedUrl)
    await fresh.locator('article').filter({ hasText: 'orchid red' }).waitFor()
    expect(JSON.parse(await fresh.getByLabel('Request inspector').innerText())).toEqual(JSON.parse(await page.getByLabel('Request inspector').innerText()))
    await fresh.getByText('Advanced controls', { exact: true }).click()
    expect(await fresh.getByLabel('Query inputs').inputValue()).toBe('{"boost":2.5}')
    expect(await fresh.getByLabel('Ranking profile').inputValue()).toBe('boosted')
    await fresh.reload()
    await fresh.locator('article').filter({ hasText: 'orchid red' }).waitFor()
    await fresh.close()
    // An empty backend page still has matches; it is distinct from no matches.
    await page.getByLabel('Offset', { exact: true }).fill('99')
    await expect.poll(async () => JSON.parse(await page.getByLabel('Request inspector').innerText()).offset?.value).toBe(99)
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.getByText('This page returned no hits.', { exact: true }).waitFor()
    expect(await page.getByRole('button', { name: 'Next page', exact: true }).isDisabled()).toBe(true)
    // Typed editor uses the SDK's canonical query shape, not another language.
    await page.getByLabel('Query mode').selectOption('typed')
    await page.getByLabel('Typed term field').selectOption('title')
    await page.getByRole('button', { name: 'Use typed term', exact: true }).click()
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.locator('article').filter({ hasText: 'orchid blue' }).waitFor()
    expect(await page.getByText(/^2 matches/).count()).toBe(1)
    expect(JSON.parse(await page.getByLabel('Request inspector').innerText()).query.source_type).toBe('TypedQuery')
    expect(JSON.parse(await page.getByLabel('Request inspector').innerText()).offset).toBeUndefined()
    const typedUrl = page.url()
    await page.getByLabel('Query mode').selectOption('text')
    await page.goBack()
    expect(page.url()).toBe(typedUrl)
    expect(await page.getByLabel('Query mode').inputValue()).toBe('typed')
    await page.goForward()
    expect(await page.getByLabel('Query mode').inputValue()).toBe('text')
    await page.getByLabel('Search this index').fill('nomatch')
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.getByText('No hits matched this query.', { exact: true }).waitFor()
    await page.getByLabel('Ranking profile').fill('missing-profile')
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.getByRole('alert').filter({ hasText: 'canonical' }).waitFor()
    expect(await page.getByRole('alert').innerText()).toContain('missing-profile')
    await page.getByRole('tab', { name: 'Error', exact: true }).click()
    expect(JSON.parse(await page.getByLabel('Error inspector').innerText()).origin).toBe('canonical')
    await page.getByLabel('Ranking profile').fill('boosted')
    await page.route(`${webEndpoint}/**`, route => route.request().method() === 'POST' ? route.abort('failed') : route.continue())
    await page.locator('form').getByRole('button', { name: 'Search', exact: true }).click()
    await page.getByRole('alert').filter({ hasText: 'transport' }).waitFor()
    expect(JSON.parse(await page.getByLabel('Error inspector').innerText()).origin).toBe('transport')
    await page.unroute(`${webEndpoint}/**`)
    await page.goto(`${ui}/indexes/workbench/search/lab?s=broken`)
    await page.getByRole('alert').filter({ hasText: 'Cannot restore Search Lab URL' }).waitFor()
    expect(errors).toEqual([])
  } finally {
    await browser?.close(); await stop(vite); await index?.close(); await stop(daemon); await rm(root, { recursive: true, force: true })
  }
}, 120_000)
