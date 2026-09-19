#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, sys
from collections import Counter
from pathlib import Path
root=Path(__file__).resolve().parents[1]
errors=[]
def load(p):
    try: return json.loads(p.read_text())
    except Exception as e: errors.append(f"{p.relative_to(root)}: {e}"); return None
manifest=load(root/'manifest.json') or {}
index=load(root/'cases.json') or {'cases':[]}
case_files=sorted((root/'cases').glob('*/*.json'))
golden_files=sorted((root/'golden').glob('*/*.json'))
allowed={
'index_definition_to_ir','field_to_ir','analyzer_to_ir','value_to_ir','document_to_ir','search_fields_to_ir','projection_to_ir','highlighting_to_ir','comparison_to_ir','range_to_ir',
'insert_to_ir','upsert_to_ir','patch_to_ir','delete_to_ir','get_to_ir','write_to_ir','filter_to_ir','filters_to_ir','sort_to_ir','facet_to_ir','ranking_to_ir','query_to_ir','search_to_ir','analyze_to_ir','describe','inspect_query'
}
expected=[]; counts=Counter(); seen=set()
for p in case_files:
    c=load(p)
    if not c: continue
    cid=c.get('id'); cat=c.get('category'); op=c.get('operation')
    if cid in seen: errors.append(f'duplicate id {cid}')
    seen.add(cid); counts[cat]+=1
    if cid != f'{cat}.{p.stem}': errors.append(f'{cid}: id/path mismatch')
    if op not in allowed: errors.append(f'{cid}: unsupported operation {op}')
    gp=root/'golden'/cat/f'{p.stem}.json'
    if not gp.exists(): errors.append(f'{cid}: missing golden'); continue
    g=load(gp)
    if g is not None:
        if set(g.keys()) not in ({'ok'},{'error'}): errors.append(f'{cid}: golden must contain exactly ok or error')
        if 'error' in g:
            e=g['error']
            if set(e.keys()) != {'code','details','retryable'}: errors.append(f'{cid}: unstable error golden shape')
            if not isinstance(e['details'],list): errors.append(f'{cid}: error.details must be list')
            for i,d in enumerate(e.get('details',[])):
                if set(d.keys()) - {'code','path'}: errors.append(f'{cid}: detail {i} contains non-stable keys')
                if 'code' not in d: errors.append(f'{cid}: detail {i} missing code')
    expected.append({'id':cid,'operation':op,'case':str(p.relative_to(root)),'golden':str(gp.relative_to(root))})
if index.get('cases') != expected: errors.append('cases.json does not exactly match active cases')
if manifest.get('case_count') != len(case_files): errors.append('manifest case_count mismatch')
if manifest.get('categories') != dict(sorted(counts.items())): errors.append('manifest categories mismatch')
expected_goldens={str((root/e['golden']).resolve()) for e in expected}
for gp in golden_files:
    if str(gp.resolve()) not in expected_goldens: errors.append(f'orphan golden: {gp.relative_to(root)}')
# Every v0.4 case must be classified.
mc=load(root/'migration/v0.4-classification.json') or {'cases':[]}
ids=[x.get('id') for x in mc.get('cases',[])]
if len(ids)!=260 or len(set(ids))!=260: errors.append('migration classification must cover exactly 260 unique v0.4 cases')
if errors:
    print('\n'.join(errors),file=sys.stderr); raise SystemExit(1)
print(f'OK: {len(case_files)} cases across {len(counts)} categories')
for k in sorted(counts): print(f'  {k}: {counts[k]}')
print('IR corpus structural invariants PASS')
