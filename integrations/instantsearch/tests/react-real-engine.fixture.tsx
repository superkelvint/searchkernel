// @vitest-environment jsdom
import { expect } from 'vitest';
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { fireEvent } from '@testing-library/react';
import * as components from 'react-instantsearch';
import { qualifyFramework, componentNames, initialParameters, pageItems, sortItems, numericItems, type Client, type FrameworkDriver } from './framework-acceptance';

export async function mount(client: Client, operator: 'or'|'and', mounted: Set<string>, advancedNumeric=false): Promise<FrameworkDriver> {
  (globalThis as any).IS_REACT_ACT_ENVIRONMENT = true;
  document.body.innerHTML = '<div id="app"></div>';
  const root = createRoot(document.querySelector('#app')!);
  let activeClient=client;
  let parameters: any = advancedNumeric ? {...initialParameters,numericFilters:['price>20','price<50']} : initialParameters;
  let current: ReturnType<typeof components.useInstantSearch>;
  let other: ReturnType<typeof components.useInstantSearch>;
  const errors: any[]=[];
  function Observer({secondary=false}: {secondary?:boolean}) {
    const value=components.useInstantSearch({catchError:true});
    if(secondary) other=value; else current=value;
    if(value.error && !errors.includes(value.error)) errors.push(value.error);
    return null;
  }
  function NumericChoices(props: any) {
    const {items,refine}=components.useNumericMenu(props);
    return <ul>{items.map(item=><li key={item.value}><button onClick={()=>refine(item.value)}>{item.label}</button></li>)}</ul>;
  }
  function Hit({hit}: {hit:any}) {
    mounted.add('Highlight'); mounted.add('Snippet');
    return <div data-id={hit.objectID}><span><components.Highlight attribute="title" hit={hit}/></span><p><components.Snippet attribute="description" hit={hit}/></p></div>;
  }
  function widget(name: keyof typeof componentNames['react-instantsearch'], id: string, props: any = {}, children?: React.ReactNode) {
    const Component=name==='numericMenu' ? NumericChoices : (components as any)[componentNames['react-instantsearch'][name]];
    expect(Component,`actual React component ${name}`).toBeTruthy(); mounted.add(name);
    return <div id={id}>{React.createElement(Component,props,children)}</div>;
  }
  async function render() {
    await act(async()=>root.render(<components.InstantSearch indexName="products" searchClient={activeClient} insights={false}>
      <Observer/>
      {widget('configure','configure',parameters)}
      {widget('searchBox','searchbox')}
      {widget('hits','hits',{hitComponent:Hit})}
      {widget('pagination','pagination')}
      {widget('hitsPerPage','perpage',{items:pageItems})}
      {widget('refinementList','brand',{attribute:'brand',operator,searchable:false})}
      {widget('menu','category',{attribute:'category'})}
      {!advancedNumeric && widget('numericMenu','numeric',{attribute:'price',items:numericItems})}
      {widget('sortBy','sort',{items:sortItems})}
      {widget('stats','stats')}
      {widget('clearRefinements','clear')}
      {widget('currentRefinements','current')}
      {widget('index','secondary',{indexName:'other'},<><Observer secondary/><components.Configure analytics={false} hitsPerPage={2}/><div id="other"><components.Hits/></div></>)}
    </components.InstantSearch>));
  }
  await render();
  return {
    results:()=>current?.results,otherResults:()=>other?.results,errors,
    state:async(value)=>{await act(async()=>current.setUiState({products:value}));},
    configure:async(value)=>{parameters=value;await render();},
    replaceClient:async(value)=>{activeClient=value;await render();},
    input:async(value)=>{await act(async()=>fireEvent.change(document.querySelector('#searchbox input')!,{target:{value}}));},
    pump:async()=>{await act(async()=>{await new Promise(resolve=>setTimeout(resolve,10));});},
    dispose:async()=>{await act(async()=>root.unmount());delete (globalThis as any).IS_REACT_ACT_ENVIRONMENT;},
  };
}
