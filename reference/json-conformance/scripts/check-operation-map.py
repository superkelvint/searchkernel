#!/usr/bin/env python3
import json
from pathlib import Path
here=Path(__file__).resolve().parent.parent
expected=json.loads((here/'expected/operation-map.json').read_text())
schema=json.loads((here/'generated/schema-model.json').read_text())

def union_payloads(name):
    for item in schema.get('enums', []):
        if item.get('kind') == 'union' and item.get('name') == name:
            return [variant['payload'] for variant in item.get('variants', []) if variant.get('payload')]
    raise SystemExit(f'{name} missing from schema-model.json')

ops=expected['operations']
for label, values in [('operation',[x['operation'] for x in ops]),('request',[x['request'] for x in ops]),('response',[x['response'] for x in ops])]:
    if len(values) != len(set(values)): raise SystemExit(f'duplicate {label}: {values}')
req=union_payloads('SearchKernel.Protocol.V2.RequestPayload')
resp=union_payloads('SearchKernel.Protocol.V2.ResponsePayload')
want_req=[x['request'] for x in ops]; want_resp=[x['response'] for x in ops]+[expected['error_response']]
if req != want_req: raise SystemExit(f'RequestPayload mismatch\nmodel={req}\nmap={want_req}')
resp_without_error=[x for x in resp if x != expected['error_response']]
if resp.count(expected['error_response']) != 1: raise SystemExit(f'ResponsePayload must contain Error exactly once: {resp}')
if resp_without_error != [x['response'] for x in ops]: raise SystemExit(f'ResponsePayload normal variants mismatch\nmodel={resp_without_error}\nmap={[x["response"] for x in ops]}')
print('C6 canonical operation inventory PASS')
