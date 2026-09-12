#!/usr/bin/env python3
from __future__ import annotations
import json, subprocess, sys
from pathlib import Path
if len(sys.argv)<2: raise SystemExit('usage: verify_adapter.py ADAPTER [CASE_PREFIX ...]')
root=Path(__file__).resolve().parents[1]; adapter=sys.argv[1]; prefixes=sys.argv[2:]
def load(p): return json.loads(p.read_text())
case_files=sorted((root/'cases').glob('*/*.json'))
if prefixes: case_files=[p for p in case_files if any(load(p)['id'].startswith(x) for x in prefixes)]
passed=0
for p in case_files:
    c=load(p); cid=c['id']; cat=c['category']
    req={'protocol':'sk-conformance/v2','case_id':cid,'operation':c['operation'],'input':c['input'],'fixtures':{}}
    if c.get('index_fixture'): req['index']=load(root/'fixtures/indexes'/f"{c['index_fixture']}.json")
    def scan(v):
        if isinstance(v,dict):
            for k,x in v.items():
                if k=='backend_response_fixture': req['fixtures'][f'backend_response:{x}']=load(root/'fixtures/backend-responses'/f'{x}.json')
                elif k=='preset_fixture': req['fixtures'][f'preset:{x}']=load(root/'fixtures/presets'/f'{x}.json')
                scan(x)
        elif isinstance(v,list):
            for x in v: scan(x)
    scan(c['input'])
    expected=load(root/'golden'/cat/(p.stem+'.json'))
    proc=subprocess.run([adapter],input=json.dumps(req,ensure_ascii=False).encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    if proc.returncode!=0:
        print(f'FAIL {cid}: adapter exit {proc.returncode}',file=sys.stderr); print(proc.stderr.decode(errors='replace'),file=sys.stderr); raise SystemExit(1)
    try: actual=json.loads(proc.stdout)
    except Exception as e:
        print(f'FAIL {cid}: invalid adapter JSON: {e}',file=sys.stderr); raise SystemExit(1)
    if actual!=expected:
        print(f'FAIL {cid}',file=sys.stderr); print('EXPECTED',json.dumps(expected,indent=2,sort_keys=True,ensure_ascii=False),file=sys.stderr); print('ACTUAL',json.dumps(actual,indent=2,sort_keys=True,ensure_ascii=False),file=sys.stderr); raise SystemExit(1)
    passed+=1
print(f'PASS: {passed} conformance cases')
