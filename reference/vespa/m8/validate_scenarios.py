#!/usr/bin/env python3
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parent
files=sorted(ROOT.glob('scenarios/[0-9][0-9]-*.json'))
if len(files)!=8: raise SystemExit(f'M8 requires exactly 8 positive scenarios, found {len(files)}')
ids=[]
for p in files:
    o=json.load(open(p,encoding='utf-8'))
    if o.get('version')!=1: raise SystemExit(f'{p}: version must be 1')
    sid=o.get('id'); ids.append(sid)
    if not isinstance(sid,str) or not sid: raise SystemExit(f'{p}: missing id')
    schema=o.get('schema',{})
    if schema.get('document_type')!='typea': raise SystemExit(f'{p}: frozen document_type must be typea')
    fields=schema.get('fields')
    if not isinstance(fields,list) or not fields: raise SystemExit(f'{p}: schema.fields must be non-empty')
    if not isinstance(o.get('documents'),list) or not o['documents']: raise SystemExit(f'{p}: documents required')
    r=o.get('request',{})
    for k in ('query','limit','offset','sort','summary'):
        if k not in r: raise SystemExit(f'{p}: request.{k} required')
    if not isinstance(r['limit'],int) or r['limit']<0: raise SystemExit(f'{p}: limit must be non-negative integer')
    if not isinstance(r['offset'],int) or r['offset']<0: raise SystemExit(f'{p}: offset must be non-negative integer')
    if not isinstance(r['sort'],list): raise SystemExit(f'{p}: sort must be list')
    for s in r['sort']:
        if set(s)!={'field','order'} or s['order'] not in ('ascending','descending'):
            raise SystemExit(f'{p}: invalid sort entry')
if len(set(ids))!=len(ids): raise SystemExit('M8 scenario ids must be unique')
byid={json.load(open(p,encoding='utf-8'))['id']:json.load(open(p,encoding='utf-8')) for p in files}
a=byid['limit-offset-sort-summary']['request']
if (a['limit'],a['offset'])!=(2,1) or not a['sort'] or a['summary']!={'fields':['qty']}:
    raise SystemExit('M8 anti-shortcut pagination/sort/summary trap changed')
if byid['limit-zero-total']['request']['limit']!=0: raise SystemExit('M8 limit-zero trap changed')
if byid['offset-past-end']['request']['offset']<=len(byid['offset-past-end']['documents']): raise SystemExit('M8 past-end offset trap changed')
stored=byid['stored-text-selected-summary']
f=stored['schema']['fields'][0]
if f.get('type')!='text' or not f.get('index') or not f.get('summary') or f.get('attribute'):
    raise SystemExit('M8 stored-text scenario must be indexed+summary and non-attribute')
if len(byid['multi-key-sort']['request']['sort'])<2: raise SystemExit('M8 multi-key sort trap changed')
for sid in ('bm25-score-default-summary','stored-text-selected-summary'):
    case=byid[sid]
    for d in case['documents']:
        for name,encoded in d['fields'].items():
            if 'text' in encoded:
                a=encoded.get('analysis')
                if not isinstance(a,list) or not a:
                    raise SystemExit(f'M8 {sid}: direct C++ oracle requires frozen M3 document analysis for {name}')
                for i,t in enumerate(a):
                    if set(t)!={'position','byte_start','byte_len','term'}:
                        raise SystemExit(f'M8 {sid}: invalid document analysis item {i}')
    q=case['request']['query'].get('term')
    if q is not None and (not isinstance(q.get('analysis'),list) or not q['analysis']):
        raise SystemExit(f'M8 {sid}: direct C++ oracle requires frozen M3 query analysis')
errs=json.load(open(ROOT/'scenarios/errors.json',encoding='utf-8'))
if errs.get('version')!=1 or len(errs.get('cases',[]))!=6: raise SystemExit('M8 requires exactly 6 frozen negative cases')
if len({c['id'] for c in errs['cases']})!=6: raise SystemExit('M8 error ids must be unique')
print('M8 scenarios valid')
