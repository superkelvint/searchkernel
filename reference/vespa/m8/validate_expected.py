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

for sp in scenarios:
    sid=json.load(open(sp,encoding='utf-8'))['id']; ep=ROOT/'expected'/f'{sid}.json'
    if not ep.is_file(): raise SystemExit(f'M8 frozen expected missing: {ep}')
    o=json.load(open(ep,encoding='utf-8'))
    if o.get('id')!=sid: raise SystemExit(f'{ep}: id mismatch')
    if not isinstance(o.get('total_hit_count'),int) or o['total_hit_count']<0: raise SystemExit(f'{ep}: invalid total_hit_count')
    hits=o.get('hits')
    if not isinstance(hits,list): raise SystemExit(f'{ep}: hits must be list')
    if o['total_hit_count']<len(hits): raise SystemExit(f'{ep}: total_hit_count < returned hits')
    for i,h in enumerate(hits):
        if set(h)!={'document_id','score_bits','summary'}: raise SystemExit(f'{ep}: hit shape changed')
        if not isinstance(h['document_id'],str): raise SystemExit(f'{ep}: hit id must be string')
        if not isinstance(h['score_bits'],str) or not bit.match(h['score_bits']): raise SystemExit(f'{ep}: exact score bits required')
        if not isinstance(h['summary'],dict): raise SystemExit(f'{ep}: summary must be object')
        for k,v in h['summary'].items(): value(v,f'{ep}: hit {i} summary {k}')
print('M8 expected output valid')
