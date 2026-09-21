#!/usr/bin/env python3
import json,re,shlex,sys
from pathlib import Path

def parse(path):
    out={}
    for raw in path.read_text().splitlines():
        line=raw.strip()
        if not line or line.startswith('#'): continue
        parts=shlex.split(line)
        if len(parts)>=2: out[parts[0]]=' '.join(parts[1:])
    return out

def as_bool(value):
    return str(value).lower()=='true'

def main():
    if len(sys.argv)!=2: raise SystemExit('usage: canonicalize_cfg.py CONFIG_DIR')
    cfg=Path(sys.argv[1])
    attrs=parse(cfg/'attributes.cfg')
    index=parse(cfg/'indexschema.cfg')
    attr_indices=sorted({int(m.group(1)) for k in attrs for m in [re.match(r'attribute\[(\d+)\]\.name$',k)] if m})
    found=None
    for i in attr_indices:
        p=f'attribute[{i}]'
        if attrs.get(f'{p}.name')=='embedding':
            found={
                'field':'embedding',
                'datatype':attrs.get(f'{p}.datatype'),
                'collection':attrs.get(f'{p}.collectiontype','SINGLE'),
                'tensor_type':attrs.get(f'{p}.tensortype'),
                'distance_metric':attrs.get(f'{p}.distancemetric'),
                'hnsw_enabled':as_bool(attrs.get(f'{p}.index.hnsw.enabled','false')),
                'max_links_per_node':int(attrs[f'{p}.index.hnsw.maxlinkspernode']),
                'neighbors_to_explore_at_insert':int(attrs[f'{p}.index.hnsw.neighborstoexploreatinsert']),
            }
            break
    if found is None: raise SystemExit('embedding tensor attribute missing from attributes.cfg')
    inverted=False
    for key,value in index.items():
        if re.match(r'indexfield\[\d+\]\.name$',key) and value=='embedding': inverted=True
    found['inverted_index_present']=inverted
    print(json.dumps(found,sort_keys=True,separators=(',',':')))
if __name__=='__main__': main()
