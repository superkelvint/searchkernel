#!/usr/bin/env python3
import json
import re
import shlex
import sys
from pathlib import Path

TYPE_NAMES = {"text", "string", "integer", "float", "boolean"}

def parse_cfg(path):
    data = {}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        parts = shlex.split(line)
        if len(parts) < 2:
            continue
        data[parts[0]] = ' '.join(parts[1:])
    return data

def as_bool(value, default=False):
    if value is None:
        return default
    return value.lower() == 'true'

def expected_manifest_fields(scenario):
    return [{
        'name': f['name'],
        'type': f['type'],
        'index': bool(f['index']),
        'attribute': bool(f['attribute']),
        'summary': bool(f['summary']),
    } for f in scenario['fields']]

def load_manifest(scenario, cfg_dir):
    path = cfg_dir / 'searchkernel-schema.json'
    if not path.is_file():
        raise SystemExit(f'missing required generated document-schema manifest: {path}')
    try:
        manifest = json.loads(path.read_text())
    except Exception as exc:
        raise SystemExit(f'invalid {path}: {exc}') from exc
    if set(manifest) != {'version', 'document_type', 'fields'}:
        raise SystemExit(f'{path}: expected exactly version/document_type/fields keys')
    if manifest['version'] != 1:
        raise SystemExit(f'{path}: version must be 1')
    if manifest['document_type'] != scenario['schema']:
        raise SystemExit(
            f"{path}: document_type {manifest['document_type']!r} != scenario {scenario['schema']!r}"
        )
    fields = manifest['fields']
    if not isinstance(fields, list):
        raise SystemExit(f'{path}: fields must be an array')
    for field in fields:
        if set(field) != {'name', 'type', 'index', 'attribute', 'summary'}:
            raise SystemExit(f'{path}: each field must contain exactly name/type/index/attribute/summary')
        if field['type'] not in TYPE_NAMES:
            raise SystemExit(f"{path}: unsupported manifest field type {field['type']!r}")
        for key in ('index','attribute','summary'):
            if type(field[key]) is not bool:
                raise SystemExit(f'{path}: {key} must be JSON boolean')
    expected = expected_manifest_fields(scenario)
    if fields != expected:
        raise SystemExit(
            f'{path}: compiled field model does not exactly match Schema input\n'
            f'expected={json.dumps(expected, sort_keys=True)}\n'
            f'actual={json.dumps(fields, sort_keys=True)}'
        )
    return manifest

def canonicalize(scenario_path, cfg_dir):
    scenario = json.loads(Path(scenario_path).read_text())
    cfg_dir = Path(cfg_dir)
    manifest = load_manifest(scenario, cfg_dir)

    required = ['indexschema.cfg', 'attributes.cfg', 'summary.cfg']
    for name in required:
        if not (cfg_dir / name).is_file():
            raise SystemExit(f'missing required generated config: {cfg_dir / name}')

    index = parse_cfg(cfg_dir / 'indexschema.cfg')
    attrs = parse_cfg(cfg_dir / 'attributes.cfg')
    summary = parse_cfg(cfg_dir / 'summary.cfg')

    index_fields = []
    indices = sorted({int(m.group(1)) for k in index for m in [re.match(r'^indexfield\[(\d+)\]\.name$', k)] if m})
    for i in indices:
        p = f'indexfield[{i}]'
        index_fields.append({
            'name': index[f'{p}.name'],
            'datatype': index.get(f'{p}.datatype', 'STRING'),
            'collection': index.get(f'{p}.collectiontype', 'SINGLE'),
            'prefix': as_bool(index.get(f'{p}.prefix'), False),
            'positions': as_bool(index.get(f'{p}.positions'), True),
            'interleaved_features': as_bool(index.get(f'{p}.interleavedfeatures'), False),
        })
    index_fields.sort(key=lambda x: x['name'])

    attributes = []
    indices = sorted({int(m.group(1)) for k in attrs for m in [re.match(r'^attribute\[(\d+)\]\.name$', k)] if m})
    for i in indices:
        p = f'attribute[{i}]'
        attributes.append({
            'name': attrs[f'{p}.name'],
            'datatype': attrs[f'{p}.datatype'],
            'collection': attrs.get(f'{p}.collectiontype', 'SINGLE'),
        })
    attributes.sort(key=lambda x: x['name'])

    classes = sorted({int(m.group(1)) for k in summary for m in [re.match(r'^classes\[(\d+)\]\.name$', k)] if m})
    default_idx = None
    for i in classes:
        if summary.get(f'classes[{i}].name') == 'default':
            default_idx = i
            break
    if default_idx is None:
        raise SystemExit('summary.cfg has no default summary class')
    pfx = f'classes[{default_idx}].fields'
    fields = []
    indices = sorted({int(m.group(1)) for k in summary for m in [re.match(re.escape(pfx) + r'\[(\d+)\]\.name$', k)] if m})
    for i in indices:
        p = f'{pfx}[{i}]'
        fields.append({
            'name': summary[f'{p}.name'],
            'command': summary.get(f'{p}.command', ''),
            'source': summary.get(f'{p}.source', ''),
        })
    fields.sort(key=lambda x: x['name'])

    return {
        'id': scenario['id'],
        'schema': manifest['document_type'],
        'index_fields': index_fields,
        'attributes': attributes,
        'summary_fields': fields,
    }

def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: canonicalize_cfg.py SCENARIO.json CONFIG_DIR')
    print(json.dumps(canonicalize(sys.argv[1], sys.argv[2]), sort_keys=True, separators=(',', ':')))

if __name__ == '__main__':
    main()
