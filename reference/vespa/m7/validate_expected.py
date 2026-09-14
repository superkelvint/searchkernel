#!/usr/bin/env python3
import json
from pathlib import Path
root=Path(__file__).resolve().parent
paths=sorted((root/'scenarios').glob('[0-9][0-9]-*.json'))
for p in paths:
    s=json.loads(p.read_text())
    e=root/'expected'/f"{s['id']}.json"
    if not e.is_file(): raise SystemExit(f'missing expected output: {e}')
    x=json.loads(e.read_text())
    if x.get('id')!=s['id'] or x.get('schema')!=s['schema'] or not isinstance(x.get('query'),dict):
        raise SystemExit(f'invalid expected output: {e}')
print(f'validated {len(paths)} M7 expected outputs')
