#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
here=Path(__file__).resolve().parent.parent; root=here.parent.parent; generated=here/'generated'
expected=json.loads((here/'expected/operation-map.json').read_text()); operations=[x['operation'] for x in expected['operations']]
def reject_dupes(pairs):
    out={}
    for k,v in pairs:
        if k in out: raise ValueError(f'duplicate key {k}')
        out[k]=v
    return out

def validate(instance,schema,root,path='$'):
    if '$ref' in schema:
        prefix='#/'
        ref=schema['$ref']
        if not ref.startswith(prefix): raise ValueError(f'{path}: unsupported reference {ref}')
        target=root
        for part in ref[len(prefix):].split('/'):
            target=target[part]
        return validate(instance,target,root,path)
    if 'const' in schema and instance != schema['const']:
        raise ValueError(f'{path}: expected {schema["const"]!r}')
    if 'enum' in schema and instance not in schema['enum']:
        raise ValueError(f'{path}: {instance!r} is not in enum')
    kind=schema.get('type')
    valid={
        'object':lambda value:isinstance(value,dict),
        'array':lambda value:isinstance(value,list),
        'string':lambda value:isinstance(value,str),
        'number':lambda value:isinstance(value,(int,float)) and not isinstance(value,bool),
        'integer':lambda value:isinstance(value,int) and not isinstance(value,bool),
        'boolean':lambda value:isinstance(value,bool),
        'null':lambda value:value is None,
    }
    if kind in valid and not valid[kind](instance):
        raise ValueError(f'{path}: expected {kind}')
    if kind=='string' and 'pattern' in schema:
        import re
        if re.fullmatch(schema['pattern'],instance) is None: raise ValueError(f'{path}: does not match pattern')
    if kind=='object' and isinstance(instance,dict):
        required=schema.get('required',[])
        missing=[name for name in required if name not in instance]
        if missing: raise ValueError(f'{path}: missing required {missing}')
        if len(instance)<schema.get('minProperties',0): raise ValueError(f'{path}: too few properties')
        if len(instance)>schema.get('maxProperties',float('inf')): raise ValueError(f'{path}: too many properties')
        properties=schema.get('properties',{})
        if schema.get('additionalProperties') is False:
            unknown=[name for name in instance if name not in properties]
            if unknown: raise ValueError(f'{path}: unknown properties {unknown}')
        for name,value in instance.items():
            if name in properties: validate(value,properties[name],root,f'{path}.{name}')
    if kind=='array' and isinstance(instance,list) and 'items' in schema:
        for index,value in enumerate(instance): validate(value,schema['items'],root,f'{path}[{index}]')
    if 'minimum' in schema and instance<schema['minimum']: raise ValueError(f'{path}: below minimum')
    if 'maximum' in schema and instance>schema['maximum']: raise ValueError(f'{path}: above maximum')
manifest=json.loads((generated/'manifest.json').read_text())
if manifest.get('generated_by')!='searchkernel-bindgen' or manifest.get('format_version')!=1: raise SystemExit('invalid manifest header')
for name in ['searchkernel_ffi_request.bfbs','searchkernel_ffi_response.bfbs']:
    want=hashlib.sha256((root/'core/crates/searchkernel-json/bfbs'/name).read_bytes()).hexdigest()
    try: got=manifest['bfbs'][name]['sha256']
    except Exception as exc: raise SystemExit(f'manifest missing hash for {name}: {exc}')
    if got != want: raise SystemExit(f'manifest hash mismatch for {name}')
schema=json.loads((generated/'searchkernel.schema.json').read_text()); openapi=json.loads((generated/'openapi-components.json').read_text())
for filename,obj in [('searchkernel.schema.json',schema),('openapi-components.json',openapi)]:
    if obj.get('x-searchkernel-generated-by')!='searchkernel-bindgen': raise SystemExit(f'{filename} missing generated marker')
    rendered=json.dumps(obj,sort_keys=True)
    for op in operations:
        if op not in rendered: raise SystemExit(f'{filename} missing operation {op}')
    for forbidden in ['payload_type','kind_type','expr_type','policy_type','operation_type','target_type']:
        if f'"{forbidden}"' in rendered: raise SystemExit(f'{filename} exposes {forbidden}')
components=openapi.get('components',{}).get('schemas')
if not isinstance(components,dict) or not components: raise SystemExit('openapi components.schemas must be non-empty')
rb=sorted((root/'core/crates/searchkernel-json/tests/fixtures/requests').glob('*.skrq')); sb=sorted((root/'core/crates/searchkernel-json/tests/fixtures/responses').glob('*.skrs'))
rj=sorted((generated/'requests').glob('*.json')); sj=sorted((generated/'responses').glob('*.json'))
if [p.stem for p in rj] != [p.stem for p in rb]: raise SystemExit('request JSON fixture names mismatch SKRQ corpus')
if [p.stem for p in sj] != [p.stem for p in sb]: raise SystemExit('response JSON fixture names mismatch SKRS corpus')
if len(rj)!=77 or len(sj)!=67: raise SystemExit(f'frozen corpus changed: {len(rj)} requests, {len(sj)} responses')
for path,envelope in [(p,'request') for p in rj]+[(p,'response') for p in sj]:
    obj=json.loads(path.read_text(),object_pairs_hook=reject_dupes)
    if obj.get('protocol_version')!=1: raise SystemExit(f'{path.name}: protocol_version')
    union=obj.get(envelope)
    if not isinstance(union,dict) or len(union)!=1: raise SystemExit(f'{path.name}: {envelope} must be one-key object')
    rendered=json.dumps(obj,sort_keys=True)
    for forbidden in ['payload_type','kind_type','expr_type','policy_type','operation_type','target_type']:
        if f'"{forbidden}"' in rendered: raise SystemExit(f'{path.name}: leaked {forbidden}')
    try:
        validate(obj,schema[envelope],schema,f'$.{envelope}')
    except (KeyError,TypeError,ValueError) as exc:
        raise SystemExit(f'{path.name}: does not validate against searchkernel.schema.json: {exc}')
print('C6 generated metadata/fixture coverage PASS')
