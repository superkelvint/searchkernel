import { describe, expect, it } from 'vitest';
import { createSearchKernelInstantSearchClient, SearchKernelInstantSearchError } from '../src/client.js';
function setup() {
  const observed: any[] = [];
  const client = createSearchKernelInstantSearchClient({ endpoint: 'http://localhost/search', fetch: (async (_url, init) => {
    const searches = JSON.parse(init!.body as string).searches;
    observed.push(searches);
    return Response.json({ results: searches.map((request: any) => ({ endpoint: request.endpoint, query: request.query,
      hits: [], totalHits: 31, exact: true, processingTimeMs: 1, facets: {}, exhaustiveFacetsCount: true })) });
  }) as typeof fetch });
  return { client, observed };
}
describe('bounded representation', () => {
  it('preserves per-request pagination, identity, and batch order', async () => {
    const { client, observed } = setup();
    const result = await client.search([{ indexName: 'one', params: { query: 'a', page: 2, hitsPerPage: 7 } },
      { indexName: 'two', params: { query: 'b', hitsPerPage: 0 } }]);
    expect(observed[0].map((r: any) => [r.endpoint, r.offset, r.limit])).toEqual([['one',14,7],['two',0,0]]);
    expect(result.results.map(r => [r.index,r.nbPages,r.page])).toEqual([['one',5,2],['two',0,0]]);
  });
  it('maps AND / OR / negation and preserves numeric operator boundaries', async () => {
    const { client, observed } = setup();
    await client.search([{ indexName: 'one', params: {
      facetFilters: [['brand:Acme', 'brand:Bravo'], 'category:-home', 'brand:\\-literal'],
      numericFilters: ['price>=10', 'price<20', ['price=15','price!=16']],
    } }]);
    const filters = observed[0][0].filters;
    expect(filters[0]).toEqual({op:'or',clauses:[{field:'brand',op:'eq',value:'Acme'},{field:'brand',op:'eq',value:'Bravo'}]});
    expect(filters.slice(1,5)).toEqual([{field:'category',op:'ne',value:'home'},{field:'brand',op:'eq',value:'-literal'},
      {field:'price',op:'gte',value:10},{field:'price',op:'lt',value:20}]);
  });
  it.each(['filters','distinct','typoTolerance','aroundLatLng','attributesToSnippet'])('rejects unsupported %s', async key => {
    const { client, observed } = setup();
    await expect(client.search([{indexName:'one',params:{[key]: key === 'attributesToSnippet' ? ['title:5'] : true}}]))
      .rejects.toMatchObject({code:'unsupported',requestIndex:0});
    expect(observed).toHaveLength(0);
  });
  it('keeps errors stable and sanitized', async () => {
    const client = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',fetch:(async () =>
      Response.json({error:{code:'backend_failure',message:'/private/path stack trace',requestIndex:1}},{status:502})) as typeof fetch});
    await expect(client.search([{indexName:'one'}])).rejects.toEqual(new SearchKernelInstantSearchError('backend_failure',1));
  });
  it('does not share slots across overlapping calls', async () => {
    const {client} = setup();
    const [a,b] = await Promise.all([client.search([{indexName:'one',params:{query:'a'}}]),client.search([{indexName:'two',params:{query:'b'}}])]);
    expect(a.results[0].query).toBe('a'); expect(b.results[0].query).toBe('b');
  });
  it('rejects unsupported methods through the stable adapter error', async () => {
    const {client} = setup();
    await expect(client.searchForFacetValues([])).rejects.toMatchObject({code:'unsupported'});
    await expect(client.getRecommendations([])).rejects.toMatchObject({code:'unsupported'});
    await expect(client.search([],{})).rejects.toMatchObject({code:'unsupported'});
  });
  it('rejects reordered results and malformed response fragments', async () => {
    const client = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',fetch:(async()=>
      Response.json({results:[{endpoint:'wrong',query:'',totalHits:0,hits:[],processingTimeMs:0}]})) as typeof fetch});
    await expect(client.search([{indexName:'one'}])).rejects.toMatchObject({code:'invalid_response'});
    const invalid = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',fetch:(async()=>
      Response.json({results:[{endpoint:'one',query:'',totalHits:1,processingTimeMs:0,hits:[{id:'x',fields:{title:'é'},highlights:{title:[{text:'é',matches:[{startByte:0,endByte:1}]}]}}]}]})) as typeof fetch});
    await expect(invalid.search([{indexName:'one'}])).rejects.toMatchObject({code:'invalid_response'});
  });
  it('bounds requests before transport and rejects malformed refinements', async () => {
    const {client,observed} = setup();
    for (const params of [{maxValuesPerFacet:0},{page:101,hitsPerPage:100},{query:'a'.repeat(4097)},
      {facetFilters:[[]]},{numericFilters:['price~~10']},{hitsPerPage:101}]) {
      await expect(client.search([{indexName:'one',params}])).rejects.toMatchObject({code:'invalid_request'});
    }
    await expect(client.search(Array.from({length:17},()=>({indexName:'one'})))).rejects.toMatchObject({code:'invalid_request'});
    expect(observed).toHaveLength(0);
  });
  it('distinguishes unavailable transport, timeout, and invalid JSON', async () => {
    const unavailable = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',fetch:(async()=>{throw Error('private');}) as typeof fetch});
    await expect(unavailable.search([{indexName:'one'}])).rejects.toMatchObject({code:'unavailable',message:'Search service unavailable'});
    const timeout = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',timeoutMs:5,fetch:((_url,init)=>new Promise((_resolve,reject)=>init!.signal!.addEventListener('abort',()=>reject(Error('private'))))) as typeof fetch});
    await expect(timeout.search([{indexName:'one'}])).rejects.toMatchObject({code:'timeout'});
    const invalid = createSearchKernelInstantSearchClient({endpoint:'http://localhost/search',fetch:(async()=>new Response('{')) as typeof fetch});
    await expect(invalid.search([{indexName:'one'}])).rejects.toMatchObject({code:'invalid_response'});
  });

});
