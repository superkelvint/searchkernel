#!/usr/bin/env python3
import json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parent
positive=sorted((ROOT/'scenarios').glob('[0-9][0-9]-*.json'))
if [p.name for p in positive] != ['01-hnsw-query-controls.json','02-hnsw-mutation-reopen.json']:
    raise SystemExit('M14 positive scenario set changed')

def validate_common(path, root):
    if root.get('version') != 1: raise SystemExit(f'{path.name}: version must be 1')
    if root.get('document_type') != 'typea': raise SystemExit(f'{path.name}: document_type changed')
    field=root.get('tensor_field',{})
    if field.get('name')!='embedding' or field.get('type_spec')!='tensor<float>(x[3])':
        raise SystemExit(f'{path.name}: frozen tensor field changed')
    if field.get('distance_metric')!='euclidean': raise SystemExit(f'{path.name}: metric must be euclidean')
    if field.get('hnsw') != {'max_links_per_node':4,'neighbors_to_explore_at_insert':16}:
        raise SystemExit(f'{path.name}: frozen HNSW construction config changed')
    docs=root.get('documents')
    if not isinstance(docs,list) or len(docs)<4: raise SystemExit(f'{path.name}: need at least four docs')
    ids=[]
    for doc in docs:
        ids.append(doc.get('id'))
        vals=doc.get('values')
        if not isinstance(vals,list) or len(vals)!=3 or not all(isinstance(v,(int,float)) and math.isfinite(v) for v in vals):
            raise SystemExit(f'{path.name}: invalid document vector')
    if len(ids)!=len(set(ids)): raise SystemExit(f'{path.name}: duplicate document id')
    q=root.get('query')
    if q != [1.0,0.0,0.0]: raise SystemExit(f'{path.name}: frozen query changed')

for path in positive:
    root=json.load(open(path,encoding='utf-8'))
    validate_common(path,root)

controls=json.load(open(ROOT/'scenarios/01-hnsw-query-controls.json',encoding='utf-8'))
steps=controls.get('steps',[])
if [s.get('id') for s in steps] != ['exact','ann','ann-extra-explore','ann-distance-threshold']:
    raise SystemExit('M14 query-control steps changed')
expected=[
    (False,3,0,None,3),
    (True,3,0,None,3),
    (True,3,5,None,3),
    (True,3,2,100.0,3),
]
actual=[(s.get('approximate'),s.get('target_hits'),s.get('explore_additional_hits'),s.get('distance_threshold'),s.get('limit')) for s in steps]
if actual != expected: raise SystemExit(f'M14 query controls changed: {actual!r}')

lifecycle=json.load(open(ROOT/'scenarios/02-hnsw-mutation-reopen.json',encoding='utf-8'))
if lifecycle.get('mutation') != {
    'update_id':'id:ns:typea::a','update_values':[-1.0,0.0,0.0],'delete_id':'id:ns:typea::b'
}:
    raise SystemExit('M14 mutation trap changed')
if lifecycle.get('query_options') != {
    'approximate':True,'target_hits':2,'explore_additional_hits':4,'distance_threshold':None,'limit':2
}:
    raise SystemExit('M14 lifecycle query options changed')

errors=json.load(open(ROOT/'scenarios/errors.json',encoding='utf-8'))
cases=errors.get('cases',[])
if errors.get('version')!=1 or len(cases)!=9: raise SystemExit('M14 requires exactly nine negative cases')
if len({c.get('id') for c in cases}) != 9: raise SystemExit('M14 negative ids must be unique')
if {c.get('path') for c in cases} != {'low_level','portable_ir','adapter'}:
    raise SystemExit('M14 negative paths changed')
required={
'hnsw_requires_tensor','hnsw_requires_attribute','invalid_hnsw_max_links','invalid_hnsw_neighbors_at_insert',
'distance_metric_requires_tensor','portable_hnsw_requires_tensor','portable_distance_metric_requires_tensor',
'hnsw_options_not_supported','attribute_options_not_supported'}
if {c.get('expected') for c in cases} != required: raise SystemExit('M14 error categories changed')
print('M14 scenarios valid')
