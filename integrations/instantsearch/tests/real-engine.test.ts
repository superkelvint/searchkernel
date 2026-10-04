// @vitest-environment jsdom
import { expect, it } from 'vitest';
import instantsearch from 'instantsearch.js';
import { searchBox, hits, pagination, hitsPerPage, refinementList, menu, numericMenu, sortBy,
  stats, clearRefinements, currentRefinements, configure, index } from 'instantsearch.js/es/widgets';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';
import compatibility from '../src/compatibility.json' with {type:'json'};

async function until(predicate: () => boolean) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > 10000) throw new Error('Timed out waiting for actual InstantSearch render');
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}

it('actual pinned widgets search, refine, paginate, browse, sort and render through real Vespa', async () => {
  const endpoint = process.env.SEARCHKERNEL_INSTANTSEARCH_ENDPOINT;
  expect(endpoint, 'acceptance requires real public Search Endpoint').toBeTruthy();
  const observed: any[] = [];
  const errors: Error[] = [];
  const client = createSearchKernelInstantSearchClient({endpoint: endpoint!, fetch: (async (url, init) => {
    observed.push(JSON.parse(init!.body as string));
    return globalThis.fetch(url, init);
  }) as typeof fetch});
  const ids = ['searchbox','hits','pagination','perpage','brand','category','numeric','sort','stats','clear','current','other'];
  document.body.innerHTML = ids.map(id => `<div id="${id}"></div>`).join('');
  const search = instantsearch({indexName:'products',searchClient:client,insights:false});
  search.on('error', event => errors.push(event.error));
  const exercised = new Set<string>();
  function covered(...names: string[]) { names.forEach(name => exercised.add(name)); }
  search.addWidgets([
    configure({hitsPerPage:6,analytics:false,attributesToHighlight:['title'],attributesToSnippet:['description']}),
    searchBox({container:'#searchbox'}),
    hits({container:'#hits',templates:{item:(hit, {html,components}) => html`<div data-id="${hit.objectID}">
      <span>${components.Highlight({attribute:'title',hit})}</span><p>${components.Snippet({attribute:'description',hit})}</p></div>`}}),
    pagination({container:'#pagination'}),
    hitsPerPage({container:'#perpage',items:[{label:'6 hits',value:6,default:true},{label:'4 hits',value:4}]}),
    refinementList({container:'#brand',attribute:'brand',operator:'or',searchable:false}),
    menu({container:'#category',attribute:'category'}),
    numericMenu({container:'#numeric',attribute:'price',items:[{label:'All'},{label:'Up to 100',end:100},{label:'Above 100',start:101}]}),
    sortBy({container:'#sort',items:[{label:'Relevance',value:'products'},{label:'Price ascending',value:'price_asc'},{label:'Price descending',value:'price_desc'}]}),
    stats({container:'#stats'}),clearRefinements({container:'#clear'}),currentRefinements({container:'#current'}),
    index({indexName:'other'}).addWidgets([configure({hitsPerPage:2,analytics:false}),hits({container:'#other'})]),
  ]);
  let renders = 0;
  search.on('render', () => renders++);
  search.start();
  try {
    await until(() => errors.length > 0 || search.helper?.lastResults?.nbHits === 24);
    expect(errors).toEqual([]);
    covered('searchBox','hits','configure','stats','index');
    expect(document.querySelectorAll('#hits .ais-Hits-item')).toHaveLength(6);
    expect(document.querySelectorAll('#other .ais-Hits-item')).toHaveLength(2);
    expect(document.querySelector('#brand')!.textContent).toContain('Acme');
    expect(search.helper!.lastResults!.getFacetValues('brand')).toEqual(expect.arrayContaining([expect.objectContaining({name:'Acme',count:12})]));
    const input = document.querySelector<HTMLInputElement>('#searchbox input')!;
    input.value = 'wireless'; input.dispatchEvent(new Event('input',{bubbles:true}));
    await until(() => search.helper!.lastResults?.query === 'wireless');
    expect(search.helper!.lastResults!.nbHits).toBe(18);
    expect(document.querySelector('#hits mark')!.textContent!.toLowerCase()).toBe('wireless');
    expect(document.querySelector('#hits')!.innerHTML).toContain('&lt;angle&gt;');
    expect(document.querySelector('#hits angle')).toBeNull();
    expect(document.querySelector('#hits p mark')).not.toBeNull();
    const description = search.helper!.lastResults!.hits[0]._snippetResult!.description.value;
    expect(description.length).toBeLessThan(1000);
    const change = async (state: Record<string,unknown>, test: () => boolean) => {
      const before = renders;
      search.setUiState({products:state,other:{}});
      await until(() => renders > before && test());
    };
    await change({query:'wireless',page:2,hitsPerPage:6}, () => search.helper!.lastResults?.page === 1);
    expect(search.helper!.lastResults!.nbPages).toBe(3); covered('pagination');
    await change({query:'wireless',page:3,hitsPerPage:6}, () => search.helper!.lastResults?.page === 2);
    expect(document.querySelectorAll('#hits .ais-Hits-item')).toHaveLength(6);
    await change({query:'wireless',hitsPerPage:4}, () => search.helper!.lastResults?.hitsPerPage === 4);
    expect(search.helper!.lastResults!.nbPages).toBe(5); covered('hitsPerPage');
    await change({query:'wireless',refinementList:{brand:['Acme']},hitsPerPage:6}, () => search.helper!.lastResults?.nbHits === 9);
    expect(search.helper!.lastResults!.getFacetValues('brand')).toEqual(expect.arrayContaining([expect.objectContaining({name:'Bravo',count:9})]));
    covered('refinementList','currentRefinements');
    await change({query:'wireless',refinementList:{brand:['Acme','Bravo']},menu:{category:'audio'}}, () => search.helper!.lastResults?.nbHits === 18);
    covered('menu');
    expect(observed.at(-1).searches.some((r:any) => r.filters.some((f:any) => f.op === 'or'))).toBe(true);
    document.querySelector<HTMLButtonElement>('#clear button')!.click();
    await until(() => !search.getUiState().products.refinementList && !search.getUiState().products.menu);
    covered('clearRefinements');
    await change({numericMenu:{price:'0:100'},sortBy:'price_asc'}, () => search.helper!.lastResults?.nbHits === 10);
    expect(search.helper!.lastResults!.hits[0].price).toBe(10); covered('numericMenu','sortBy');
    await change({sortBy:'price_desc'}, () => search.helper!.lastResults?.hits[0]?.price === 240);
    await change({query:'nonexistentzz'}, () => search.helper!.lastResults?.nbHits === 0);
    expect(document.querySelectorAll('#hits .ais-Hits-item')).toHaveLength(0);
    await change({}, () => search.helper!.lastResults?.nbHits === 24);
    const batch = await client.search([{indexName:'products',params:{query:'wireless',hitsPerPage:3,page:1}},
      {indexName:'other',params:{query:'speaker',hitsPerPage:2}}, {indexName:'products',params:{query:'speaker',hitsPerPage:0}}]);
    expect(batch.results.map(r => [r.index,r.query,r.nbHits,r.hits.length])).toEqual([
      ['products','wireless',18,3],['other','speaker',6,2],['products','speaker',6,0]]);
    const beyond = await client.search([{indexName:'products',params:{page:99,hitsPerPage:6}}]);
    expect(beyond.results[0]).toMatchObject({nbHits:24,nbPages:4,page:99,hits:[]});
    await expect(client.search([{indexName:'missing'}])).rejects.toMatchObject({code:'unknown_index',requestIndex:0});
    await expect(client.search([{indexName:'products',params:{filters:'brand:Acme'}}])).rejects.toMatchObject({code:'unsupported'});
    const malformed = await fetch(endpoint!,{method:'POST',headers:{'Content-Type':'application/json'},body:'{"searches":['});
    expect(malformed.status).toBe(400);
    expect(await malformed.json()).toMatchObject({error:{code:'invalid_request'}});
    const isolated = await fetch(endpoint!.replace('/search','/v1/execute'),{method:'POST',body:'{}'});
    expect(isolated.ok).toBe(false);
    await expect(client.search([{indexName:'stale'}])).rejects.toMatchObject({code:'stale_target',requestIndex:0});
    await expect(client.search([{indexName:'products',params:{attributesToSnippet:['missing']}}])).rejects.toMatchObject({code:'unsupported'});
    const mixedFailure = await client.search([{indexName:'products'}, {indexName:'missing'}]).catch(error=>error);
    expect(mixedFailure).toMatchObject({code:'unknown_index',requestIndex:1});
    const numeric = await client.search([{indexName:'products',params:{numericFilters:['price>100','price<130']}},
      {indexName:'products',params:{facetFilters:['brand:-Acme']}},
      {indexName:'products',params:{query:'wireless headphones',attributesToHighlight:['title'],attributesToSnippet:['description']}}]);
    expect(numeric.results.map(r=>r.nbHits)).toEqual([2,12,18]);
    expect(numeric.results[2].hits[0]._highlightResult.title.matchedWords.map(w=>w.toLowerCase())).toEqual(expect.arrayContaining(['wireless','headphones']));
    expect(numeric.results[2].hits[0]._snippetResult.description.matchLevel).toBe('partial');
    const typed = await client.search([{indexName:'products',params:{facets:['stock'],facetFilters:['stock:true'],numericFilters:['rating>1.0','rating<1.3']}}]);
    expect(typed.results[0].nbHits).toBe(1);
    expect(typed.results[0].hits[0].rating).toBe(1.1);
    expect(typed.results[0].facets.stock).toEqual({'true':1});
    document.body.insertAdjacentHTML('beforeend','<div id="andbrand"></div><div id="andhits"></div>');
    const conjunction = instantsearch({indexName:'products',searchClient:client,insights:false});
    conjunction.addWidgets([configure({analytics:false}),refinementList({container:'#andbrand',attribute:'brand',operator:'and'}),hits({container:'#andhits'})]);
    conjunction.start();
    try {
      await until(()=>conjunction.helper!.lastResults?.nbHits===24);
      conjunction.setUiState({products:{refinementList:{brand:['Acme','Bravo']}}});
      await until(()=>conjunction.helper!.lastResults?.nbHits===0);
    } finally { conjunction.dispose(); }
    const missingField = await client.search([{indexName:'price_desc',params:{query:'speaker',attributesToSnippet:['description']}}]);
    expect(missingField.results[0].hits[0].objectID).toBe('products-24');
    expect(missingField.results[0].hits[0]._snippetResult.description).toBeUndefined();
    const browse = await client.search([{indexName:'products',params:{attributesToHighlight:['title'],attributesToSnippet:['description']}}]);
    expect(browse.results[0].hits[0]._highlightResult.title.matchLevel).toBe('none');
    expect(browse.results[0].hits[0]._snippetResult.description.matchLevel).toBe('none');
    for (const body of [JSON.stringify({searches:[]}),JSON.stringify({searches:Array.from({length:17},()=>observed[0].searches[0])}),
      JSON.stringify({searches:[{...observed[0].searches[0],query:'a'.repeat(65536)}]})]) {
      const response = await fetch(endpoint!,{method:'POST',headers:{'Content-Type':'application/json'},body});
      expect(response.status).toBe(400);
      expect(await response.json()).toMatchObject({error:{code:'invalid_request'}});
    }
    input.value='speaker'; input.dispatchEvent(new Event('input',{bubbles:true}));
    input.value='wireless'; input.dispatchEvent(new Event('input',{bubbles:true}));
    await until(()=>search.helper!.lastResults?.query==='wireless');
    expect(search.helper!.lastResults!.nbHits).toBe(18);
    expect(errors).toEqual([]);
    for (const [name, widget] of Object.entries(compatibility.widgets)) {
      if (widget.status !== 'unsupported') expect(exercised.has(name),`matrix support without actual widget acceptance: ${name}`).toBe(true);
    }
  } finally { search.dispose(); }
}, 60000);
