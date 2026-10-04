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
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor().catch(async error => { console.error('Workspace failure:', await page.locator('body').innerText()); throw error })
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
    let detailUrl = page.url()
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
      const staleUrl = detailUrl
      await page.goto(staleUrl)
      await page.getByText(/Continuing with this handle is unsafe/).waitFor()
      const refreshed = new URL(detailUrl); refreshed.searchParams.set('generation', index.generation); detailUrl = refreshed.toString()
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

test.skipIf(process.env.SEARCHKERNEL_FRONTEND_REAL_ENGINE !== '1')('W4 browser lifecycle diagnostics isolation stale recovery and restart', async () => {
  const root = await mkdtemp(resolve(tmpdir(), 'searchkernel-web-w4-'))
  const grpcPort = await port(), webPort = await port(), uiPort = await port()
  const endpoint = `127.0.0.1:${grpcPort}`, webEndpoint = `http://127.0.0.1:${webPort}`, ui = `http://127.0.0.1:${uiPort}`
  let daemon: ChildProcess | undefined, vite: ChildProcess | undefined, browser: Browser | undefined
  const handles: Index[] = []
  async function start() {
    daemon = spawn(resolve(process.env.CARGO_TARGET_DIR ?? '../target/host', 'debug/searchkerneld'), ['--data-root', root, '--listen', endpoint, '--web-listen', `127.0.0.1:${webPort}`, '--web-origin', ui], { stdio: 'inherit' })
    await reachable(webEndpoint, daemon)
  }
  try {
    await start()
    vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', String(uiPort)], { env: { ...process.env, VITE_SEARCHKERNEL_ENDPOINT: webEndpoint }, stdio: 'inherit' })
    await reachable(ui, vite)
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] })
    const page = await browser.newPage({ permissions: ['clipboard-read', 'clipboard-write'] })
    await page.goto(`${ui}/indexes`)
    await page.getByText('No persisted indexes.', { exact: true }).waitFor()
    await page.getByText(/Ready · version/).waitFor()
    await page.goto(`${ui}/indexes/missing/overview`)
    await page.getByText('not_found', { exact: true }).waitFor()
    await page.getByRole('button', { name: 'Refresh inventory / select current incarnation', exact: true }).click()
    await page.getByText('No persisted indexes.', { exact: true }).waitFor()
    const definition = JSON.stringify({ name: 'w4_schema', fields: [{ name: 'title', data_type: { kind_type: 'PrimitiveDataType', kind: { kind: 'String' } }, stored: { value: true }, searchable: { value: true }, matching: { mode: 'Text' } }] })
    async function edit(name: string, json = definition) {
      await page.goto(`${ui}/indexes/new`)
      await page.getByLabel('Index target', { exact: true }).fill(name)
      await page.getByRole('button', { name: 'Edit canonical definition', exact: true }).click()
      await page.getByLabel('Canonical IndexDefinition').fill(json)
    }
    async function validate() {
      await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
      await page.getByText('Validation passed', { exact: true }).waitFor()
      await page.getByRole('checkbox', { name: /I confirm applying/ }).check()
    }
    async function create(name: string) {
      await edit(name); await validate()
      await page.getByRole('button', { name: 'Create index', exact: true }).click()
      await page.getByRole('heading', { name: 'Complete canonical definition', exact: true }).waitFor()
      await page.goto(`${ui}/indexes`)
      await page.getByRole('article', { name: `Index ${name}`, exact: true }).waitFor()
    }
    await edit('alpha', JSON.stringify({ ...JSON.parse(definition), fields: [...JSON.parse(definition).fields, ...JSON.parse(definition).fields] }))
    await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
    await page.getByRole('alert').filter({ hasText: 'invalid_argument' }).waitFor()
    await create('alpha'); await create('beta')
    // Canonical create must refuse a target created after validation by another client.
    await edit('race'); await validate()
    const racer = await Index.create(endpoint, 'race', new IndexDefinitionT('w4_schema', [new Text({ stored: true, searchable: true }).definition('title')]))
    handles.push(racer)
    await page.getByRole('button', { name: 'Create index', exact: true }).click()
    await page.getByRole('alert').filter({ hasText: 'already_exists' }).waitFor()
    await Index.delete(endpoint, 'race')
    await page.goto(`${ui}/indexes`)
    const alpha = await Index.open(endpoint, 'alpha'), beta = await Index.open(endpoint, 'beta')
    handles.push(alpha, beta)
    expect(alpha.generation).not.toBe(beta.generation)
    await alpha.upsert('one', { title: 'old incarnation' }); await beta.upsert('sibling', { title: 'durable sibling' })
    await page.getByRole('button', { name: 'alpha', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor().catch(async error => { console.error('Workspace failure:', await page.locator('body').innerText()); throw error })
    const oldUrl = `${ui}/indexes/alpha/operations/diagnostics?generation=${encodeURIComponent(alpha.generation)}`
    await page.goto(oldUrl)
    await page.getByText('Lifecycle: open', { exact: true }).waitFor()
    await page.getByRole('button', { name: 'Refresh index status', exact: true }).click()
    await page.getByText('Persisted: true', { exact: true }).waitFor()
    await page.getByText('Index diagnostics', { exact: true }).last().click()
    const diagnostics = JSON.parse(await page.getByLabel('Index diagnostics inspector').innerText())
    expect(diagnostics.status).toMatchObject({ name: 'alpha', generation: alpha.generation, persisted: true, state: 'open', schema_name: 'w4_schema' })
    expect(diagnostics.definition.index.name).toBe('w4_schema')
    await page.getByRole('button', { name: 'Copy Index diagnostics', exact: true }).click()
    expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(diagnostics)
    await page.addScriptTag({ type: 'module', content: `import { createDaemonSearchKernelClient } from '/src/api/daemon-searchkernel-client.ts'; import { IndexTargetT } from '/src/api/searchkernel-client.ts'; window.w4client = createDaemonSearchKernelClient(${JSON.stringify(webEndpoint)}); window.w4old = new IndexTargetT('alpha', ${JSON.stringify(alpha.generation)});` })
    await page.waitForFunction(() => !!(window as any).w4client)
    expect(await page.evaluate(async () => (await (window as any).w4client.getDocument((window as any).w4old, 'one')).fields.title)).toBe('old incarnation')
    await page.goto(`${ui}/indexes`)
    const row = page.getByRole('article', { name: 'Index alpha', exact: true })
    await row.getByRole('button', { name: 'Delete index', exact: true }).click().catch(async error => { console.error('Delete inventory failure:', await page.locator('body').innerText()); throw error })
    const dialog = row.getByRole('dialog')
    expect(await dialog.innerText()).toContain(alpha.generation)
    expect(await dialog.getByRole('button', { name: 'Permanently delete index', exact: true }).isDisabled()).toBe(true)
    await dialog.getByLabel('Confirm index name').fill('beta')
    expect(await dialog.getByRole('button', { name: 'Permanently delete index', exact: true }).isDisabled()).toBe(true)
    await dialog.getByLabel('Confirm index name').fill('alpha')
    await dialog.getByRole('button', { name: 'Permanently delete index', exact: true }).click()
    await row.waitFor({ state: 'detached' })
    const discovery = new Daemon(endpoint)
    try { expect(await discovery.listIndexes()).toEqual(['beta']) } finally { await discovery.close() }
    expect((await beta.get('sibling'))?.title).toBe('durable sibling')
    await expect(alpha.get('one')).rejects.toMatchObject({ code: 'not_found' })
    await expect(Index.open(endpoint, 'alpha')).rejects.toMatchObject({ code: 'not_found' })
    await create('alpha')
    let replacement = await Index.open(endpoint, 'alpha'); handles.push(replacement)
    expect(replacement.generation).not.toBe(alpha.generation)
    await replacement.upsert('one', { title: 'new incarnation' })
    await expect(alpha.get('one')).rejects.toMatchObject({ code: 'failed_precondition' })
    // Normal public adapter path encodes the retained target; no wire payload surgery.
    await page.addScriptTag({ type: 'module', content: `import { createDaemonSearchKernelClient } from '/src/api/daemon-searchkernel-client.ts'; import { IndexTargetT } from '/src/api/searchkernel-client.ts'; window.w4client = createDaemonSearchKernelClient(${JSON.stringify(webEndpoint)}); window.w4old = new IndexTargetT('alpha', ${JSON.stringify(alpha.generation)});` })
    await page.waitForFunction(() => !!(window as any).w4client)
    const stale = await page.evaluate(async () => { try { await (window as any).w4client.getDocument((window as any).w4old, 'one'); return null } catch (error) { const e = error as any; return { code: e.code, details: e.details, origin: e.origin } } })
    expect(stale).toMatchObject({ code: 'failed_precondition', origin: 'canonical' })
    expect(JSON.stringify(stale)).toContain('stale_index_handle')
    await page.goto(oldUrl)
    await page.getByText(/Continuing with this handle is unsafe/).waitFor()
    expect(page.url()).toContain(alpha.generation)
    await page.getByRole('button', { name: 'Refresh inventory / select current incarnation', exact: true }).click()
    await page.getByRole('button', { name: 'alpha', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor().catch(async error => { console.error('Workspace failure:', await page.locator('body').innerText()); throw error })
    expect(page.url()).toContain(replacement.generation)
    await page.goto(`${ui}/indexes/alpha/data/documents?generation=${replacement.generation}`)
    await page.getByRole('button', { name: 'Open one', exact: true }).click()
    await page.getByRole('dialog').getByText('new incarnation', { exact: true }).waitFor()
    expect(await page.getByRole('dialog').getByText('old incarnation', { exact: true }).count()).toBe(0)
    // A confirmation must remain pinned if another client replaces the index.
    await page.goto(`${ui}/indexes`)
    const replaceRow = page.getByRole('article', { name: 'Index alpha', exact: true })
    await replaceRow.getByRole('button', { name: 'Delete index', exact: true }).click()
    await replaceRow.getByLabel('Confirm index name').fill('alpha')
    await Index.delete(endpoint, 'alpha')
    replacement = await Index.create(endpoint, 'alpha', new IndexDefinitionT('w4_schema', [new Text({ stored: true, searchable: true }).definition('title')]))
    handles.push(replacement)
    await replacement.upsert('one', { title: 'third incarnation' })
    await replaceRow.getByRole('button', { name: 'Permanently delete index', exact: true }).click()
    await replaceRow.getByRole('alert').filter({ hasText: 'stale_index_handle' }).waitFor()
    expect((await replacement.get('one'))?.title).toBe('third incarnation')
    expect((await beta.get('sibling'))?.title).toBe('durable sibling')
    await replaceRow.getByRole('button', { name: 'Cancel', exact: true }).click()
    // Slow open A must not disable B or override the user's later selection.
    let lifecycleRequests = 0
    await page.route(`${webEndpoint}/**`, async route => {
      if (route.request().url().endsWith('/Execute') && route.request().method() === 'POST') await new Promise(done => setTimeout(done, ++lifecycleRequests === 1 ? 1500 : 100))
      await route.continue()
    })
    const slowOpen = page.waitForRequest(request => request.url().endsWith('/Execute'))
    await replaceRow.getByRole('button', { name: 'alpha', exact: true }).click()
    const pendingOpen = await slowOpen
    const betaRow = page.getByRole('article', { name: 'Index beta', exact: true })
    expect(await betaRow.getByRole('button', { name: 'beta', exact: true }).isEnabled()).toBe(true)
    await betaRow.getByRole('button', { name: 'beta', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor()
    await (await pendingOpen.response())?.finished()
    await page.waitForTimeout(300) // Let the completed lifecycle callback reconcile its inactive observers.
    expect(page.url()).toContain(`/indexes/beta/overview?generation=${beta.generation}`)
    await page.unroute(`${webEndpoint}/**`)
    // A delayed alpha response cannot populate beta's workbench.
    await page.route(`${webEndpoint}/**`, async route => { if (route.request().method() === 'POST') await new Promise(done => setTimeout(done, 250)); await route.continue() })
    const inFlight = page.waitForRequest(request => request.url().endsWith('/Execute') && request.method() === 'POST')
    await page.goto(`${ui}/indexes/alpha/data/documents?generation=${replacement.generation}`)
    await inFlight
    await page.evaluate(url => { history.pushState({}, '', url); dispatchEvent(new PopStateEvent('popstate')) }, `/indexes/beta/data/documents?generation=${beta.generation}`)
    await page.getByRole('button', { name: 'Open sibling', exact: true }).waitFor()
    expect(await page.getByRole('button', { name: 'Open one', exact: true }).count()).toBe(0)
    await page.unroute(`${webEndpoint}/**`)
    await page.goto(`${ui}/operations`)
    await page.getByText('Ready', { exact: true }).waitFor()
    await Promise.all(handles.map(index => index.close())); await stop(daemon)
    await page.getByRole('button', { name: 'Refresh health', exact: true }).click()
    await page.getByText('Daemon unavailable. Retry or refresh health.', { exact: true }).waitFor()
    await start()
    await page.getByRole('button', { name: 'Refresh health', exact: true }).click()
    await page.getByText('Ready', { exact: true }).waitFor()
    await page.goto(`${ui}/indexes`)
    await page.getByRole('button', { name: 'Refresh inventory and health', exact: true }).click()
    await page.getByRole('article', { name: 'Index beta', exact: true }).getByText('persisted', { exact: true }).waitFor()
    await page.getByRole('button', { name: 'beta', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor().catch(async error => { console.error('Workspace failure:', await page.locator('body').innerText()); throw error })
    expect(page.url()).not.toContain(beta.generation)
    const reopened = await Index.open(endpoint, 'beta'); handles.push(reopened)
    expect((await reopened.get('sibling'))?.title).toBe('durable sibling')
    await page.goto(`${ui}/indexes/beta/data/documents?generation=${beta.generation}`)
    await page.getByText(/Continuing with this handle is unsafe/).waitFor()
    expect(page.url()).toContain(beta.generation)
    await page.getByRole('button', { name: 'Refresh inventory / select current incarnation', exact: true }).click()
    await page.getByRole('button', { name: 'beta', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor()
    await page.goto(`${ui}/indexes/beta/data/documents?generation=${reopened.generation}`)
    await page.getByRole('button', { name: 'Open sibling', exact: true }).click()
    await page.getByRole('dialog').getByText('durable sibling', { exact: true }).waitFor()
    const reopenedAlpha = await Index.open(endpoint, 'alpha'); handles.push(reopenedAlpha)
    expect(reopenedAlpha.generation).not.toBe(replacement.generation)
    expect((await reopenedAlpha.get('one'))?.title).toBe('third incarnation')
    await page.goto(`${ui}/indexes/alpha/data/documents?generation=${reopenedAlpha.generation}`)
    await page.getByRole('button', { name: 'Open one', exact: true }).click()
    await page.getByRole('dialog').getByText('third incarnation', { exact: true }).waitFor()
    expect((await reopened.get('sibling'))?.title).toBe('durable sibling')
  } finally {
    await browser?.close(); await stop(vite); await Promise.all(handles.map(index => index.close())); await stop(daemon); await rm(root, { recursive: true, force: true })
  }
}, 180_000)

test.skipIf(process.env.SEARCHKERNEL_FRONTEND_REAL_ENGINE !== '1')('schema and ranking administration validates, applies and persists the supported slice', async () => {
  const root = await mkdtemp(resolve(tmpdir(), 'searchkernel-configuration-browser-'))
  const grpcPort = await port(), webPort = await port(), uiPort = await port()
  const endpoint = `127.0.0.1:${grpcPort}`, webEndpoint = `http://127.0.0.1:${webPort}`
  let daemon: ChildProcess | undefined, vite: ChildProcess | undefined, browser: Browser | undefined
  async function start() {
    daemon = spawn(resolve(process.env.CARGO_TARGET_DIR ?? '../target/host', 'debug/searchkerneld'),
      ['--data-root', root, '--listen', endpoint, '--web-listen', `127.0.0.1:${webPort}`, '--web-origin', `http://127.0.0.1:${uiPort}`], { stdio: 'inherit' })
    await reachable(webEndpoint, daemon)
  }
  try {
    await start()
    vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', String(uiPort)], {
      env: { ...process.env, VITE_SEARCHKERNEL_ENDPOINT: webEndpoint }, stdio: 'inherit',
    })
    await reachable(`http://127.0.0.1:${uiPort}`, vite)
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] })
    const page = await browser.newPage()
    await page.goto(`http://127.0.0.1:${uiPort}/indexes/new`)
    await page.getByRole('textbox', { name: 'Index target', exact: true }).fill('catalog')
    await page.getByRole('textbox', { name: 'Schema name', exact: true }).fill('products')
    for (const [position, field] of ([['sku', 'keyword'], ['stock', 'int'], ['price', 'float'], ['active', 'bool']] as const).entries()) {
      await page.getByRole('button', { name: 'Add field', exact: true }).click()
      await page.getByRole('textbox', { name: `Field ${position + 2} name`, exact: true }).fill(field[0])
      await page.getByRole('combobox', { name: 'Type', exact: true }).nth(position + 1).selectOption(field[1])
    }
    await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
    await page.getByText('Validation passed', { exact: true }).waitFor()
    expect(await page.getByRole('button', { name: 'Create index', exact: true }).isEnabled()).toBe(false)
    await page.getByRole('checkbox', { name: /I confirm applying/ }).check()
    await page.getByRole('button', { name: 'Create index', exact: true }).click()
    await page.getByRole('heading', { name: 'Complete canonical definition', exact: true }).waitFor()
    const writer = await Index.open(endpoint, 'catalog')
    await writer.upsert('one', { title: 'orchid', sku: 'orchid-1', stock: 4n, price: 1.5, active: true })
    await writer.close()
    await page.goto(`http://127.0.0.1:${uiPort}/indexes/catalog/configuration/ranking`)
    await page.getByRole('button', { name: 'Create profile', exact: true }).click()
    await page.getByRole('textbox', { name: 'Profile name', exact: true }).fill('custom')
    await page.getByRole('textbox', { name: 'First-phase expression', exact: true }).fill('1')
    await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
    await page.getByText('Validation passed', { exact: true }).waitFor()
    // Editing after validation removes the apply control immediately.
    await page.getByRole('textbox', { name: 'First-phase expression', exact: true }).fill('2')
    expect(await page.getByRole('button', { name: 'Apply validated change', exact: true }).count()).toBe(0)
    await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
    await page.getByText('Validation passed', { exact: true }).waitFor()
    await page.getByRole('checkbox', { name: /I confirm applying/ }).check()
    await page.getByRole('button', { name: 'Apply validated change', exact: true }).click()
    await page.getByRole('button', { name: 'custom', exact: true }).waitFor()
    await page.locator('input[readonly]').waitFor()
    await page.getByRole('textbox', { name: 'First-phase expression', exact: true }).fill('invalid_function(')
    await page.getByRole('button', { name: 'Validate draft', exact: true }).click()
    await page.getByRole('alert').waitFor()
    expect(await page.getByRole('button', { name: 'Apply validated change', exact: true }).count()).toBe(0)
    await stop(daemon); await start()
    const current = await Index.open(endpoint, 'catalog')
    await page.reload()
    await page.getByText(/Continuing with this handle is unsafe/).waitFor()
    await page.goto(`http://127.0.0.1:${uiPort}/indexes`)
    await page.getByRole('button', { name: 'catalog', exact: true }).click()
    await page.getByRole('button', { name: 'Search documents', exact: true }).waitFor()
    await page.goto(`http://127.0.0.1:${uiPort}/indexes/catalog/configuration/ranking?generation=${current.generation}`)
    await current.close()
    await page.getByRole('textbox', { name: 'First-phase expression', exact: true }).waitFor()
    expect(await page.getByRole('textbox', { name: 'First-phase expression', exact: true }).inputValue()).toBe('2')
    const reopened = await Index.open(endpoint, 'catalog')
    try {
      expect(await reopened.get('one')).toMatchObject({ title: 'orchid', sku: 'orchid-1', stock: 4n, price: 1.5, active: true })
      const result = await reopened.search('orchid').ranking('custom').execute()
      expect(result.hits[0]?.score).toBe(2)
    } finally { await reopened.close() }
  } finally {
    await browser?.close(); await stop(vite); await stop(daemon)
    await rm(root, { recursive: true, force: true })
  }
}, 120_000)
