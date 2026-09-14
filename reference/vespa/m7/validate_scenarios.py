#!/usr/bin/env python3
import json, re
from pathlib import Path
ROOT=Path(__file__).resolve().parent
NAME=re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")

def die(m): raise SystemExit(m)

def validate_query(q,path):
    if not isinstance(q,dict) or "kind" not in q: die(f"{path}: invalid typed query")
    k=q["kind"]
    if k in ("term","phrase"):
        if not NAME.fullmatch(q.get("field","")) or not isinstance(q.get("text"),str) or not q["text"]: die(f"{path}: bad {k}")
    elif k in ("and","or"):
        if not isinstance(q.get("children"),list): die(f"{path}: bad children")
        for c in q["children"]: validate_query(c,path)
    elif k=="not": validate_query(q.get("child"),path)
    elif k=="equals":
        if not NAME.fullmatch(q.get("field","")) or not isinstance(q.get("value"),int): die(f"{path}: bad equals")
    elif k=="range":
        if not NAME.fullmatch(q.get("field","")): die(f"{path}: bad range field")
        for side in ("lower","upper"):
            b=q.get(side)
            if not isinstance(b,dict) or b.get("kind") not in ("unbounded","included","excluded"): die(f"{path}: bad range bound")
            if b["kind"]!="unbounded" and not isinstance(b.get("value"),int): die(f"{path}: bad range value")
    elif k=="exists":
        if not NAME.fullmatch(q.get("field","")): die(f"{path}: bad exists")
    else: die(f"{path}: unsupported typed query kind {k!r}")

ids=set()
paths=sorted((ROOT/'scenarios').glob('[0-9][0-9]-*.json'))
for p in paths:
    x=json.loads(p.read_text())
    if x.get('id') in ids: die(f'duplicate id {x.get("id")}')
    ids.add(x['id'])
    if not NAME.fullmatch(x.get('schema','')): die(f'{p}: bad schema')
    fields={}
    for f in x.get('fields',[]):
        n=f.get('name','')
        if not NAME.fullmatch(n) or n in fields: die(f'{p}: bad/duplicate field')
        if f.get('type') not in ('text','integer'): die(f'{p}: M7 scenario field type outside verified subset')
        fields[n]=f
    validate_query(x['typed'],p)
    if not isinstance(x.get('yql'),str) or not x['yql'].strip(): die(f'{p}: missing yql')
    if not isinstance(x.get('expected_hits'),list) or len(set(x['expected_hits']))!=len(x['expected_hits']): die(f'{p}: bad expected_hits')
    docids=[]
    for d in x.get('documents',[]):
        if d.get('id') in docids: die(f'{p}: duplicate document id')
        docids.append(d.get('id'))
        if d.get('kind') not in ('text','integer'): die(f'{p}: bad document kind')
        if d.get('field') not in fields: die(f'{p}: document field unknown')
    if not set(x['expected_hits']) <= set(docids): die(f'{p}: expected hit missing from docs')

err=json.loads((ROOT/'scenarios/errors.json').read_text())
for c in err.get('cases',[]):
    if c.get('id') in ids: die(f'duplicate error id {c.get("id")}')
    ids.add(c['id'])
    if 'typed' in c: validate_query(c['typed'],'errors.json')
    if 'typed' not in c and 'yql' not in c: die('error case has no path')
    for k in ('typed_error','yql_error'):
        if k in c and c[k] not in ('unknown_field','wrong_field_type','invalid_query','invalid_yql','unsupported_yql'):
            die(f'unknown error category {c[k]}')
print(f'validated {len(paths)} M7 valid scenarios and {len(err.get("cases",[]))} error cases')
