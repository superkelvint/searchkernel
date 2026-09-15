#!/usr/bin/env python3
import json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parent
scenarios=sorted(ROOT.glob('scenarios/[0-9][0-9]-*.json'))
bit=re.compile(r'^0x[0-9a-f]{16}$')

def value(v,where):
    if not isinstance(v,dict) or 'type' not in v: raise SystemExit(f'{where}: canonical typed value required')
    t=v['type']
    if t=='float':
        if set(v)!={'type','bits'} or not isinstance(v['bits'],str) or not bit.match(v['bits']): raise SystemExit(f'{where}: invalid float bits')
    elif t in ('string','integer','boolean'):
        if set(v)!={'type','value'}: raise SystemExit(f'{where}: invalid {t} value')
    else: raise SystemExit(f'{where}: unknown value type {t!r}')

def hit(h,where):
    if set(h)!={'document_id','score_bits','summary'}: raise SystemExit(f'{where}: hit shape changed')
    if not bit.match(h['score_bits']): raise SystemExit(f'{where}: score must be exact bits')
    for k,v in h['summary'].items(): value(v,f'{where}.summary.{k}')

for sp in scenarios:
    s=json.load(open(sp,encoding='utf-8')); sid=s['id']; ep=ROOT/'expected'/f'{sid}.json'
    if not ep.is_file(): raise SystemExit(f'M9 frozen expected missing: {ep}')
    o=json.load(open(ep,encoding='utf-8'))
    if o.get('id')!=sid or not isinstance(o.get('steps'),list): raise SystemExit(f'{ep}: invalid root')
    expected_ids=[x['id'] for x in s['steps']]
    if [x.get('id') for x in o['steps']]!=expected_ids: raise SystemExit(f'{ep}: output steps must preserve scenario step order')
    for x in o['steps']:
        op=x.get('op')
        if op=='update':
            if x.get('ok') is not True: raise SystemExit(f'{ep}: update must report ok=true')
        elif op=='search':
            if not isinstance(x.get('total_hit_count'),int) or not isinstance(x.get('hits'),list): raise SystemExit(f'{ep}: invalid search step')
            if x['total_hit_count']<len(x['hits']): raise SystemExit(f'{ep}: invalid total hit count')
            for i,h in enumerate(x['hits']): hit(h,f'{ep}: hit {i}')
        elif op=='group':
            groups=x.get('groups')
            if not isinstance(groups,list): raise SystemExit(f'{ep}: groups must be list')
            for i,g in enumerate(groups):
                if set(g)!={'key','count','aggregations'}: raise SystemExit(f'{ep}: group shape changed')
                value(g['key'],f'{ep}: group {i} key')
                if not isinstance(g['count'],int) or g['count']<0: raise SystemExit(f'{ep}: invalid group count')
                if not isinstance(g['aggregations'],list): raise SystemExit(f'{ep}: aggregations must be list')
                for j,v in enumerate(g['aggregations']): value(v,f'{ep}: group {i} aggregation {j}')
        else: raise SystemExit(f'{ep}: unknown output op {op!r}')
print('M9 expected output valid')
