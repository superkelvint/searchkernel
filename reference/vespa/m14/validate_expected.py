#!/usr/bin/env python3
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
cfg=json.load(open(ROOT/'expected/hnsw-config.json',encoding='utf-8'))
if cfg != {
  'field':'embedding','datatype':'TENSOR','collection':'SINGLE','tensor_type':'tensor<float>(x[3])',
  'distance_metric':'EUCLIDEAN','hnsw_enabled':True,'max_links_per_node':4,
  'neighbors_to_explore_at_insert':16,'inverted_index_present':False
}:
    raise SystemExit('M14 frozen config golden changed')
q=json.load(open(ROOT/'expected/hnsw-query-controls.json',encoding='utf-8'))
if q.get('id')!='hnsw-query-controls' or [s.get('id') for s in q.get('steps',[])] != ['exact','ann','ann-extra-explore','ann-distance-threshold']:
    raise SystemExit('M14 query golden shape changed')
for step in q['steps']:
    if step.get('top_hit')!='id:ns:typea::a' or step.get('hit_count')!=3 or step.get('forbidden')!=[]:
        raise SystemExit('M14 query golden invariants changed')
l=json.load(open(ROOT/'expected/hnsw-mutation-reopen.json',encoding='utf-8'))
if l.get('id')!='hnsw-mutation-reopen': raise SystemExit('M14 lifecycle golden id changed')
for key,top in [('before_update','id:ns:typea::a'),('after_update','id:ns:typea::b'),('after_reopen','id:ns:typea::b')]:
    if l.get(key,{}).get('top_hit')!=top: raise SystemExit(f'M14 {key} top hit changed')
for key in ['after_delete','after_delete_reopen']:
    if l.get(key,{}).get('forbidden') != []:
        raise SystemExit(f'M14 {key} deletion invariant changed')
print('M14 expected outputs valid')
