#!/usr/bin/env python3
import json,sys

def key(v):
 t=v.get('type')
 if t=='string': return ('0',v.get('value',''))
 if t=='integer': return ('1',int(v.get('value',0)))
 if t=='boolean': return ('2',1 if v.get('value') else 0)
 if t=='float': return ('3',v.get('bits',''))
 return ('9',json.dumps(v,sort_keys=True,separators=(',',':')))
def walk(x):
 if isinstance(x,list): return [walk(v) for v in x]
 if not isinstance(x,dict): return x
 y={k:walk(v) for k,v in x.items()}
 if y.get('op')=='group' and isinstance(y.get('groups'),list): y['groups']=sorted(y['groups'],key=lambda g:key(g['key']))
 return y
obj=walk(json.load(sys.stdin))
print(json.dumps(obj,sort_keys=True,separators=(',',':')))
