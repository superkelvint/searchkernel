import { expect } from 'vitest';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';
import contract from '../src/compatibility.json';

export type Client = ReturnType<typeof createSearchKernelInstantSearchClient>;
export interface FrameworkDriver {
  results(): any;
  otherResults(): any;
  state(value: Record<string, unknown>): Promise<void>;
  configure(value: Record<string, unknown>): Promise<void>;
  replaceClient(client: Client): Promise<void>;
  input(value: string): Promise<void>;
  pump(): Promise<void>;
  dispose(): Promise<void>;
  errors: any[];
}
import { frameworkCases } from './framework-registry.mjs';
export const componentNames = Object.fromEntries(Object.entries(frameworkCases).map(([name,entry])=>[name,entry.components])) as Record<'react-instantsearch'|'vue-instantsearch',Record<string,string>>;
export const sortItems = [{label:'Relevance',value:'products'},{label:'Price ascending',value:'price_asc'}, {label:'Price descending',value:'price_desc'}, {label:'Missing',value:'missing'}, {label:'Invalid sort',value:'invalid_sort'}];
export const pageItems = [{label:'6 hits',value:6,default:true},{label:'4 hits',value:4}];
export const numericItems = [{label:'All'},{label:'Up to 100',end:100},{label:'Above 100',start:101}];
export const initialParameters = {hitsPerPage:6,analytics:false,facets:['*'],attributesToHighlight:['title'],attributesToSnippet:['description']};

export async function qualifyFramework(
  framework: keyof typeof componentNames,
  mount: (client: Client, operator: 'or' | 'and', mounted: Set<string>, advancedNumeric?: boolean) => Promise<FrameworkDriver>,
) {
  const endpoint = process.env.SEARCHKERNEL_INSTANTSEARCH_ENDPOINT;
  expect(endpoint, 'requires the real native public endpoint').toBeTruthy();
  const observed: any[] = [];
  const responses: any[] = [];
  let replacements = 0;
  let delayOldResponse = false;
  const makeClient = (replacement = false) => createSearchKernelInstantSearchClient({endpoint:endpoint!,fetch: (async (url, init) => {
    if (replacement) replacements++;
    observed.push(JSON.parse(init!.body as string));
    const response = await globalThis.fetch(url,init);
    if (delayOldResponse && JSON.parse(init!.body as string).searches.some((r:any)=>r.query==='speaker')) await new Promise(resolve=>setTimeout(resolve,100));
    if (response.ok) responses.push(await response.clone().json());
    return response;
  }) as typeof fetch});
  const mounted = new Set<string>();
  const driver = await mount(makeClient(),'or',mounted);
  const exercised = new Set<string>();
  const cover = (...names: string[]) => names.forEach(name => exercised.add(name));
  async function until(predicate: () => boolean) {
    const started = Date.now();
    while (!predicate()) {
      if (Date.now() - started > 10000) throw new Error(`${framework} acceptance timed out; results=${JSON.stringify({query:driver.results()?.query,hits:driver.results()?.nbHits,page:driver.results()?.page})}; errors=${driver.errors.map(error=>error.message)}`);
      await driver.pump();
    }
    await driver.pump();
  }
  async function state(value: Record<string,unknown>, predicate: () => boolean) {
    const count = responses.length;
    await driver.state(value);
    await until(() => responses.length > count && predicate());
  }
  try {
    await until(() => driver.results()?.nbHits === 24 && driver.otherResults()?.nbHits === 12);
    expect(driver.errors).toEqual([]);
    expect(document.querySelector('#hits mark')).toBeNull();
    expect(document.querySelectorAll('#hits .ais-Hits-item')).toHaveLength(6);
    expect(document.querySelectorAll('#other .ais-Hits-item')).toHaveLength(2);
    expect(driver.results().getFacetValues('brand')).toEqual(expect.arrayContaining([expect.objectContaining({name:'Acme',count:12}),expect.objectContaining({name:'Bravo',count:12})]));
    expect(document.querySelector('#brand')!.textContent).toContain('12');
    expect(document.querySelector('#stats')!.textContent).toContain('24');
    cover('hits','index','configure','stats');
    await driver.input('wireless');
    await until(() => driver.results()?.query === 'wireless' && driver.results()?.nbHits === 18);
    cover('searchBox');
    expect(document.querySelector('#hits mark')!.textContent!.toLowerCase()).toBe('wireless');
    expect(document.querySelector('#hits')!.innerHTML).toContain('&lt;angle&gt;');
    expect(document.querySelector('#hits angle')).toBeNull();
    expect(document.querySelector('#hits p img')).toBeNull();
    expect(document.querySelector('#hits p')!.textContent).toContain('<img src=x>');
    expect(document.querySelector('#hits p mark')).not.toBeNull();
    cover('Highlight','Snippet');
    // Drive real component events, then check backend page metadata.
    const secondPage = [...document.querySelectorAll<HTMLAnchorElement>('#pagination a')].find(a => a.textContent === '2')!;
    expect(secondPage).toBeTruthy(); secondPage.click();
    await until(() => driver.results()?.page === 1);
    expect(driver.results().nbPages).toBe(3); cover('pagination');
    const select = document.querySelector<HTMLSelectElement>('#perpage select')!;
    select.value = '4'; select.dispatchEvent(new Event('change',{bubbles:true}));
    await until(() => driver.results()?.hitsPerPage === 4);
    expect(driver.results().nbPages).toBe(5); cover('hitsPerPage');
    await state({query:'wireless',page:5,hitsPerPage:4}, () => driver.results()?.page === 4);
    expect(driver.results().hits).toHaveLength(2);
    await state({query:'wireless',refinementList:{brand:['Acme']},hitsPerPage:6}, () => driver.results()?.nbHits === 9);
    expect(driver.results().getFacetValues('brand')).toEqual(expect.arrayContaining([expect.objectContaining({name:'Bravo',count:9})]));
    expect(document.querySelector('#current')!.textContent).toContain('Acme'); cover('currentRefinements');
    await state({query:'wireless',refinementList:{brand:['Acme','Bravo']},menu:{category:'audio'}}, () => driver.results()?.nbHits === 18);
    expect(observed.at(-1).searches.some((request:any)=>request.filters.some((filter:any)=>filter.op==='or'))).toBe(true);
    await state({menu:{category:'home'},refinementList:{brand:['Acme','Bravo']}},()=>driver.results()?.nbHits===6);
    expect(driver.results().hits.every((hit:any)=>hit.category==='home')).toBe(true);
    await state({menu:{category:'home'},refinementList:{brand:['Acme']}},()=>driver.results()?.nbHits===3);
    await state({query:'wireless',refinementList:{brand:['Acme','Bravo']},menu:{category:'audio'}},()=>driver.results()?.nbHits===18);
    cover('refinementList','menu');
    document.querySelector<HTMLButtonElement>('#clear button')!.click();
    await until(() => driver.results()?.nbHits === 18 && driver.results().getRefinements().length === 0);
    expect(document.querySelector('#current')!.textContent).not.toContain('Acme'); cover('clearRefinements');
    if(framework==='react-instantsearch') [...document.querySelectorAll<HTMLButtonElement>('#numeric button')].find(button=>button.textContent==='Up to 100')!.click();
    else document.querySelectorAll<HTMLInputElement>('#numeric input')[1].click();
    await until(()=>driver.results()?.nbHits===10);
    await state({numericMenu:{price:'0:100'},sortBy:'price_asc'}, () => driver.results()?.nbHits === 10 && driver.results()?.hits[0]?.price===10);
    cover('numericMenu','sortBy');
    await state({sortBy:'price_desc'}, () => driver.results()?.hits[0]?.price === 240);
    expect(driver.results().hits[0]._snippetResult?.description).toBeUndefined();
    expect(document.querySelector('#hits p')!.textContent).toBe('');
    const baseline = responses.length;
    await driver.configure({...initialParameters,facetFilters:['brand:-Acme']});
    await until(() => responses.length>baseline && driver.results()?.nbHits===12);
    expect(driver.results().hits.every((hit:any)=>hit.brand==='Bravo')).toBe(true);
    const beforeReset=responses.length;
    await driver.configure(initialParameters);
    await until(()=>responses.length>beforeReset && driver.results()?.nbHits===24);
    await state({query:'no-such-product'}, () => driver.results()?.nbHits===0);
    expect(document.querySelectorAll('#hits .ais-Hits-item')).toHaveLength(0);
    await state({query:'wireless headphones'}, () => driver.results()?.nbHits===18);
    expect(document.querySelectorAll('#hits span mark').length).toBeGreaterThanOrEqual(2);
    await state({},()=>driver.results()?.nbHits===24);
    expect(driver.otherResults().hits.every((hit:any)=>hit.objectID.startsWith('other-'))).toBe(true);
    expect(driver.results().hits.every((hit:any)=>hit.objectID.startsWith('products-'))).toBe(true);
    expect(observed.some(batch=>batch.searches.some((r:any)=>r.endpoint==='products')&&batch.searches.some((r:any)=>r.endpoint==='other'))).toBe(true);
    // React supports prop replacement. Vue 4.30.0 qualifies a stable client:
    // its watcher updates helper.client, not the main helper that executes.
    if(framework==='react-instantsearch') {
      await driver.replaceClient(makeClient(true));
      await until(()=>replacements>0 && driver.results()?.nbHits===24);
    }
    delayOldResponse=true;
    const beforeOverlap=observed.length;
    await driver.state({query:'speaker'});
    await until(()=>observed.slice(beforeOverlap).some(batch=>batch.searches.some((r:any)=>r.query==='speaker')));
    await driver.state({query:'wireless'});
    await until(()=>driver.results()?.query==='wireless' && driver.results()?.nbHits===18);
    for(let i=0;i<20;i++) await driver.pump();
    expect(driver.results().query).toBe('wireless');
    expect(driver.otherResults().query).toBe('wireless');
    await driver.state({sortBy:'missing'});
    await until(()=>driver.errors.some(error=>error.code==='unknown_index'));
    expect(driver.errors.find(error=>error.code==='unknown_index').message).not.toContain('stack');
    await driver.state({sortBy:'invalid_sort'});
    await until(()=>driver.errors.some(error=>error.code==='invalid_request'));
    await driver.state({query:'speaker'});
    await until(()=>driver.results()?.query==='speaker' && driver.results()?.nbHits===6);
    await driver.configure({...initialParameters,clickAnalytics:true});
    await until(()=>driver.errors.some(error=>error.code==='unsupported'));
    await driver.configure(initialParameters);
    await until(()=>driver.results()?.nbHits===6);
    if(framework==='vue-instantsearch') {
      const beforeReplacement=observed.length;
      await driver.replaceClient(makeClient(true));
      await until(()=>observed.length>beforeReplacement);
      expect(replacements,'Vue pinned watcher does not replace the executing main client').toBe(0);
    }
    // Contract-to-runtime coverage: actual exported components must be mounted
    // and corresponding behavior asserted. Never infer acceptance from filenames.
    for (const [name, capability] of Object.entries({...contract.widgets,...contract.helpers})) {
      if (capability.status !== 'unsupported') {
        expect(mounted.has(name),`${framework} component missing: ${name}`).toBe(true);
        expect(exercised.has(name),`${framework} behavior missing: ${name}`).toBe(true);
      }
    }
  } finally { await driver.dispose(); }
  const advanced = await mount(makeClient(),'or',new Set(),true);
  try {
    for(let i=0;i<1000 && advanced.results()?.nbHits!==2;i++) await advanced.pump();
    expect(advanced.results().hits.map((hit:any)=>hit.price)).toEqual([30,40]);
    expect(observed.at(-1).searches[0].filters).toEqual(expect.arrayContaining([expect.objectContaining({op:'gt'}),expect.objectContaining({op:'lt'})]));
  } finally {await advanced.dispose();}
  const conjunction = await mount(makeClient(),'and',new Set());
  try {
    for(let i=0;i<1000 && conjunction.results()?.nbHits!==24;i++) await conjunction.pump();
    await conjunction.state({refinementList:{brand:['Acme','Bravo']}});
    for(let i=0;i<1000 && conjunction.results()?.nbHits!==0;i++) await conjunction.pump();
    expect(conjunction.results().nbHits).toBe(0);
    expect(observed.at(-1).searches[0].filters.some((filter:any)=>filter.op==='or')).toBe(false);
  } finally {await conjunction.dispose();}
}
