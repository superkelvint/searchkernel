import instantsearch from 'instantsearch.js';
import { configure, searchBox, hits, pagination, hitsPerPage, refinementList, menu, numericMenu,
  sortBy, stats, clearRefinements, currentRefinements } from 'instantsearch.js/es/widgets';
import { createSearchKernelInstantSearchClient } from '@searchkernel/instantsearch';

const search = instantsearch({
  indexName: 'products', insights: false,
  searchClient: createSearchKernelInstantSearchClient({endpoint:new URL('/search',location.href).href}),
});
search.addWidgets([
  configure({hitsPerPage:6,analytics:false,attributesToHighlight:['title'],attributesToSnippet:['description']}),
  searchBox({container:'#searchbox'}),
  hits({container:'#hits',templates:{item:(hit,{html,components}) => html`<article data-id="${hit.objectID}">
    <h2>${components.Highlight({attribute:'title',hit})}</h2>
    <p>${components.Snippet({attribute:'description',hit})}</p><span class="price">${hit.price}</span></article>`}}),
  pagination({container:'#pagination'}),
  hitsPerPage({container:'#perpage',items:[{label:'6 hits',value:6,default:true},{label:'4 hits',value:4}]}),
  refinementList({container:'#brand',attribute:'brand',operator:'or',searchable:false}),
  menu({container:'#category',attribute:'category'}),
  numericMenu({container:'#numeric',attribute:'price',items:[{label:'All prices'},{label:'Up to 100',end:100},{label:'Above 100',start:101}]}),
  sortBy({container:'#sort',items:[{label:'Relevance',value:'products'},{label:'Price ascending',value:'price_asc'},
    {label:'Price descending',value:'price_desc'},{label:'Intentional canonical sort error',value:'invalid_sort'},{label:'Intentional unknown index',value:'missing'}]}),
  stats({container:'#stats'}),clearRefinements({container:'#clear'}),currentRefinements({container:'#current'}),
]);
search.on('render',() => {
  const result = search.helper?.lastResults;
  if (!result) return;
  const output = document.querySelector<HTMLOutputElement>('#metadata')!;
  output.dataset.query = result.query; output.dataset.count = String(result.nbHits);
  output.dataset.page = String(result.page); output.dataset.pages = String(result.nbPages);
  output.dataset.size = String(result.hitsPerPage);
  output.textContent = `${result.nbHits} results, page ${result.page+1}/${result.nbPages}`;
});
search.on('error', event => {
  document.querySelector('#error')!.textContent = `${event.error.code}: ${event.error.message}`;
});
search.start();
