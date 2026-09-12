#!/usr/bin/env python3
import json
from pathlib import Path

root = Path(__file__).resolve().parent
scenarios = sorted((root / 'scenarios').glob('[0-9][0-9]-*.json'))
for scenario_path in scenarios:
    scenario = json.loads(scenario_path.read_text())
    expected = root / 'expected' / f"{scenario['id']}.json"
    if not expected.is_file():
        raise SystemExit(f'missing expected output: {expected}')
    obj = json.loads(expected.read_text())
    if obj.get('id') != scenario['id'] or obj.get('schema') != scenario['schema']:
        raise SystemExit(f'invalid expected identity: {expected}')
    for key in ('index_fields','attributes','summary_fields'):
        if not isinstance(obj.get(key), list):
            raise SystemExit(f'{expected}: {key} must be a list')

base = json.loads((root / 'expected' / 'text-summary.json').read_text())
alt = json.loads((root / 'expected' / 'products-text-summary.json').read_text())
for key in ('index_fields','attributes','summary_fields'):
    if base[key] != alt[key]:
        raise SystemExit(f'document-type pair should have identical Java-derived {key}')
if base['schema'] == alt['schema']:
    raise SystemExit('document-type pair expected outputs must retain distinct schema identity')

print(f'validated {len(scenarios)} M6 expected outputs')
