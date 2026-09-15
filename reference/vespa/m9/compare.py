#!/usr/bin/env python3
import json,sys

def key(v):
    t=v.get('type')
    if t=='string': return ('0-string',v.get('value',''))
    if t=='integer': return ('1-integer',int(v.get('value',0)))
    if t=='boolean': return ('2-boolean',1 if v.get('value') else 0)
    if t=='float': return ('3-float',v.get('bits',''))
    return ('9-unknown',json.dumps(v,sort_keys=True,separators=(',',':')))

def canon(o):
    if isinstance(o,list): return [canon(x) for x in o]
    if not isinstance(o,dict): return o
    x={k:canon(v) for k,v in o.items()}
    if x.get('op')=='group' and isinstance(x.get('groups'),list):
        x['groups']=sorted(x['groups'],key=lambda g:key(g['key']))
    return x

if len(sys.argv)!=3: raise SystemExit('usage: compare.py EXPECTED ACTUAL')
a=canon(json.load(open(sys.argv[1],encoding='utf-8'))); b=canon(json.load(open(sys.argv[2],encoding='utf-8')))
if a!=b:
 print('EXPECTED:',json.dumps(a,indent=2,sort_keys=True),file=sys.stderr)
 print('ACTUAL:',json.dumps(b,indent=2,sort_keys=True),file=sys.stderr)
 raise SystemExit(1)
