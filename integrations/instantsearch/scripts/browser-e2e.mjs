import { createServer } from 'node:http';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium } from 'playwright';

const backend = process.env.SEARCHKERNEL_INSTANTSEARCH_ENDPOINT;
assert.ok(backend, 'Browser acceptance requires a real Search Kernel public endpoint');
const bundle = await build({entryPoints:['fixtures/application.ts'],bundle:true,write:false,format:'iife',platform:'browser'});
const ids = ['searchbox','hits','pagination','perpage','brand','category','numeric','sort','stats','clear','current'];
const html = `<!doctype html><html><body>${ids.map(id=>`<div id="${id}"></div>`).join('')}
  <output id="metadata"></output><output id="error"></output><script src="/app.js"></script></body></html>`;
const requests = [];
// An ordinary application host with a reverse proxy; no query/response mocks,
// adapter hooks, disabled browser security, or UI-side search implementation.
const server = createServer(async (request,response) => {
  if (request.url === '/search' && request.method === 'POST') {
    const chunks = []; for await (const chunk of request) chunks.push(chunk);
    const body = Buffer.concat(chunks);
    requests.push(JSON.parse(body));
    try {
      const upstream = await fetch(backend,{method:'POST',headers:{'Content-Type':'application/json'},body});
      response.writeHead(upstream.status,{'Content-Type':'application/json'});
      response.end(Buffer.from(await upstream.arrayBuffer()));
    } catch { response.writeHead(503); response.end('{}'); }
  } else if (request.url === '/app.js') {
    response.writeHead(200,{'Content-Type':'text/javascript'}); response.end(bundle.outputFiles[0].contents);
  } else if (request.url === '/') {
    response.writeHead(200,{'Content-Type':'text/html'}); response.end(html);
  } else { response.writeHead(404); response.end(); }
});
server.listen(0,'127.0.0.1'); await once(server,'listening');
let browser;
try {
  browser = await chromium.launch({headless:true,args:['--no-sandbox']});
  const page = await browser.newPage();
  const pageErrors = []; page.on('pageerror',error=>pageErrors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const state = async values => page.waitForFunction(expected => {
    const meta = document.querySelector('#metadata');
    return meta && Object.entries(expected).every(([key,value])=>meta.dataset[key] === String(value));
  },values,{timeout:10000});
  await state({query:'',count:24,page:0,size:6,pages:4});
  assert.equal(await page.locator('#hits article').count(),6);
  await page.locator('#searchbox input').fill('wireless');
  await state({query:'wireless',count:18,pages:3});
  assert.equal((await page.locator('#hits h2 mark').first().textContent()).toLowerCase(),'wireless');
  assert.equal(await page.locator('#hits angle').count(),0);
  assert.ok((await page.locator('#hits h2').first().textContent()).includes('<angle> &'));
  assert.ok(await page.locator('#hits p mark').count());
  await page.locator('#pagination a').getByText('2',{exact:true}).click();
  await state({page:1});
  await page.locator('#pagination a').getByText('3',{exact:true}).click();
  await state({page:2});
  assert.equal(await page.locator('#hits article').count(),6);
  await page.locator('#perpage select').selectOption('4');
  await state({size:4,pages:5,page:0});
  const acme = page.locator('#brand .ais-RefinementList-item').filter({hasText:'Acme'});
  await acme.locator('input').check(); await state({count:9});
  assert.ok((await page.locator('#current').textContent()).includes('Acme'));
  const bravo = page.locator('#brand .ais-RefinementList-item').filter({hasText:'Bravo'});
  await bravo.locator('input').check(); await state({count:18});
  await page.locator('#category .ais-Menu-item').filter({hasText:'audio'}).locator('a').click();
  await page.waitForFunction(()=>document.querySelector('#current').textContent.includes('audio'));
  assert.ok(requests.at(-1).searches.some(search=>search.filters.some(filter=>filter.op==='or')));
  await page.locator('#numeric .ais-NumericMenu-item').filter({hasText:'Up to 100'}).locator('label').click();
  await state({count:10});
  await page.locator('#clear button').click(); await state({count:18});
  await page.locator('#sort select').selectOption('price_asc');
  await page.waitForFunction(()=>document.querySelector('#hits .price')?.textContent==='10');
  await page.locator('#sort select').selectOption('price_desc');
  await page.waitForFunction(()=>document.querySelector('#hits .price')?.textContent==='180');
  await page.locator('#searchbox input').fill('nonexistentzz'); await state({count:0,pages:0});
  assert.equal(await page.locator('#hits article').count(),0);
  await page.locator('#searchbox input').fill('wireless'); await state({count:18});
  await page.locator('#searchbox input').fill(''); await state({count:24});
  await page.locator('#sort select').selectOption('invalid_sort');
  await page.waitForFunction(()=>document.querySelector('#error').textContent==='invalid_request: Invalid search request');
  await page.locator('#sort select').selectOption('missing');
  await page.waitForFunction(()=>document.querySelector('#error').textContent.includes('unknown_index: Unknown search index'));
  assert.deepEqual(pageErrors,[]);
  console.log('Chromium E2E: browse/query/hits/pagination/page size/facets/OR/AND/numeric/clear/sorts/highlights/snippets/counts/zero/recovery/stable error: PASS');
} finally {
  await browser?.close(); server.closeAllConnections(); await new Promise(resolve=>server.close(resolve));
}
