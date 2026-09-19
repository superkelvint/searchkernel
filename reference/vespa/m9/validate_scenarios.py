#!/usr/bin/env python3
import json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parent
files=sorted(ROOT.glob('scenarios/[0-9][0-9]-*.json'))
if len(files)!=14: raise SystemExit(f'M9 requires exactly 14 positive scenarios, found {len(files)}')
ids=[]; group_count=0; update_count=0
for p in files:
    o=json.load(open(p,encoding='utf-8'))
    if o.get('version')!=1: raise SystemExit(f'{p}: version must be 1')
    sid=o.get('id'); ids.append(sid)
    if not isinstance(sid,str) or not sid: raise SystemExit(f'{p}: missing id')
    schema=o.get('schema',{})
    if schema.get('document_type')!='typea': raise SystemExit(f'{p}: document_type must be typea')
    types={f['name']:f['type'] for f in schema.get('fields',[])}
    for required,t in [('category','string'),('qty','integer'),('price','float'),('active','boolean')]:
        if types.get(required)!=t: raise SystemExit(f'{p}: common attribute field {required}/{t} missing')
    if not o.get('documents'): raise SystemExit(f'{p}: documents required')
    steps=o.get('steps')
    if not isinstance(steps,list) or not steps: raise SystemExit(f'{p}: steps required')
    seen=set()
    for step in steps:
        stepid=step.get('id')
        if not isinstance(stepid,str) or stepid in seen: raise SystemExit(f'{p}: step ids must be unique')
        seen.add(stepid)
        op=step.get('op')
        if op=='group':
            group_count+=1
            r=step.get('request',{})
            if set(('query','group_by','aggregations'))-set(r): raise SystemExit(f'{p}: incomplete group request')
            if any(k in r for k in ('limit','hits','offset')): raise SystemExit(f'{p}: grouping must not request regular hits')
            for aggregation in r['aggregations']:
                if aggregation != 'count' and not (isinstance(aggregation,dict) and set(aggregation)=={'average'} and isinstance(aggregation['average'],str)):
                    raise SystemExit(f'{p}: unsupported grouping aggregation {aggregation!r}')
        elif op=='search':
            r=step.get('request',{})
            if 'query' not in r: raise SystemExit(f'{p}: search query missing')
        elif op=='update':
            update_count+=1
            if not step.get('document_id') or not step.get('assign'): raise SystemExit(f'{p}: incomplete update')
        else:
            raise SystemExit(f'{p}: unsupported step op {op!r}')
if len(set(ids))!=14: raise SystemExit('M9 scenario ids must be unique')
if group_count<6: raise SystemExit('M9 must retain broad native grouping coverage')
if update_count<3: raise SystemExit('M9 must retain update-before/after coverage')
facet=json.load(open(ROOT/'scenarios/07-string-facet-counts.json',encoding='utf-8'))
if len(facet.get('documents',[])) < 25: raise SystemExit('M9 facet completeness trap must exceed ordinary 10-hit page')
ints=json.load(open(ROOT/'scenarios/10-integer-aggregations-by-category.json',encoding='utf-8'))
if 'count' not in ints['steps'][0]['request']['aggregations']:
    raise SystemExit('M9 integer grouping fixture must request count explicitly')
rank=json.load(open(ROOT/'scenarios/06-attribute-ranking-after-update.json',encoding='utf-8'))
fixture=rank.get('fixture_rank_profile')
if fixture!={'name':'attribute_price','expression':'attribute(price)'}:
    raise SystemExit('M9 native ranking-access fixture changed')
for required in ('string-facet-counts','filtered-facet-counts','integer-aggregations-by-category','float-aggregations-by-category','grouping-after-update'):
    if required not in ids: raise SystemExit(f'M9 required grouping trap missing: {required}')
composite=json.load(open(ROOT/'scenarios/14-composite-attribute-grouping.json',encoding='utf-8'))
query=composite['steps'][0]['request']['query']
if set(query)!= {'and'} or len(query['and'])!=2 or 'attribute_equals' not in query['and'][0] or 'attribute_range' not in query['and'][1]:
    raise SystemExit('M9 composite scenario must combine attribute equality and range')
if composite['steps'][0]['op']!='group':
    raise SystemExit('M9 composite scenario must use native grouping')
errs=json.load(open(ROOT/'scenarios/errors.json',encoding='utf-8'))
if errs.get('version')!=1 or len(errs.get('cases',[]))!=27: raise SystemExit('M9 requires exactly 27 frozen negative cases')
if len({c['id'] for c in errs['cases']})!=27: raise SystemExit('M9 error ids must be unique')
print('M9 scenarios valid')
