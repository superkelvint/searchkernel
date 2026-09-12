#!/usr/bin/env python3
from __future__ import annotations
import json, subprocess, sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
if len(sys.argv)!=2:
    print('usage: verify_adapter.py /path/to/adapter',file=sys.stderr); raise SystemExit(2)
adapter=sys.argv[1]
manifest=json.loads((root/'manifest.json').read_text())
for ent in json.loads((root/'cases.json').read_text())['cases']:
    c=json.loads((root/ent['case']).read_text()); g=json.loads((root/ent['golden']).read_text())
    idx={}
    if 'index_fixture' in c:
        idx=json.loads((root/'fixtures/indexes'/f"{c['index_fixture']}.json").read_text())
    payload={'protocol':manifest['adapter_protocol'],'case_id':c['id'],'operation':c['operation'],'index':idx,'input':c.get('input',{})}
    p=subprocess.run([adapter],input=json.dumps(payload)+'\n',text=True,capture_output=True)
    if p.returncode!=0:
        print(f"{c['id']}: adapter exit {p.returncode}: {p.stderr}",file=sys.stderr); raise SystemExit(1)
    try: got=json.loads(p.stdout)
    except Exception as e:
        print(f"{c['id']}: invalid adapter JSON: {e}: {p.stdout!r}",file=sys.stderr); raise SystemExit(1)
    if got!=g:
        print(f"{c['id']}: mismatch",file=sys.stderr)
        print('expected '+json.dumps(g,sort_keys=True),file=sys.stderr)
        print('got      '+json.dumps(got,sort_keys=True),file=sys.stderr)
        raise SystemExit(1)
print(f"PASS: {manifest['case_count']} IR conformance cases")
