// @vitest-environment jsdom
import { expect } from 'vitest';
import { createApp, h, nextTick, shallowRef } from 'vue';
import * as components from 'vue-instantsearch/vue3/es';
import { qualifyFramework, componentNames, initialParameters, pageItems, sortItems, numericItems, type Client, type FrameworkDriver } from './framework-acceptance';

export async function mount(client:Client,operator:'or'|'and',mounted:Set<string>,advancedNumeric=false):Promise<FrameworkDriver> {
  document.body.innerHTML='<div id="app"></div>';
  const activeClient=shallowRef(client);
  const parameters=shallowRef<Record<string,unknown>>(advancedNumeric ? {...initialParameters,numericFilters:['price>20','price<50']} : initialParameters);
  let search:any;
  const errors:any[]=[];
  const middleware=({instantSearchInstance}:any)=>{
    search=instantSearchInstance;
    const onError=({error}:any)=>errors.push(error);
    return {subscribe(){search.on('error',onError);},unsubscribe(){search.removeListener('error',onError);},onStateChange(){}};
  };
  function widget(name:keyof typeof componentNames['vue-instantsearch'],id:string,props:any={},slots?:any) {
    const Component=(components as any)[componentNames['vue-instantsearch'][name]];
    expect(Component,`actual Vue component ${name}`).toBeTruthy();mounted.add(name);
    return h('div',{id},[h(Component,props,slots)]);
  }
  const app=createApp({render:()=>h(components.AisInstantSearch,{indexName:'products',searchClient:activeClient.value,insights:false,middlewares:[middleware]}, {default:()=>[
    widget('configure','configure',parameters.value),widget('searchBox','searchbox'),
    widget('hits','hits',{}, {item:({item}:any)=>{
      mounted.add('Highlight');mounted.add('Snippet');
      return h('div',{'data-id':item.objectID},[h('span',{},[h(components.AisHighlight,{attribute:'title',hit:item})]),h('p',{},[h(components.AisSnippet,{attribute:'description',hit:item})])]);
    }}),
    widget('pagination','pagination'),widget('hitsPerPage','perpage',{items:pageItems}),
    widget('refinementList','brand',{attribute:'brand',operator,searchable:false}),widget('menu','category',{attribute:'category'}),
    ...(!advancedNumeric ? [widget('numericMenu','numeric',{attribute:'price',items:numericItems})] : []),widget('sortBy','sort',{items:sortItems}),widget('stats','stats'),
    widget('clearRefinements','clear'),widget('currentRefinements','current'),
    widget('index','secondary',{indexName:'other'},{default:()=>[h(components.AisConfigure,{analytics:false,hitsPerPage:2}),h('div',{id:'other'},[h(components.AisHits)])]}),
  ]})});
  app.mount('#app');await nextTick();
  return {
    results:()=>search?.helper?.lastResults,
    otherResults:()=>search?.mainIndex.getWidgets().find((widget:any)=>widget.getIndexId?.()==='other')?.getResults(),errors,
    state:async(value)=>{search.setUiState({products:value});await nextTick();},
    configure:async(value)=>{parameters.value=value;await nextTick();},
    replaceClient:async(value)=>{activeClient.value=value;await nextTick();},
    input:async(value)=>{const input=document.querySelector<HTMLInputElement>('#searchbox input')!;input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));await nextTick();},
    pump:async()=>{await new Promise(resolve=>setTimeout(resolve,10));await nextTick();},
    dispose:async()=>{app.unmount();await nextTick();},
  };
}
