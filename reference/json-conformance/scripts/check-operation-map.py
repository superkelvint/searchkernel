#!/usr/bin/env python3
import json,re,sys
from pathlib import Path
here=Path(__file__).resolve().parent.parent; root=here.parent.parent
expected=json.loads((here/'expected/operation-map.json').read_text()); generated=json.loads((here/'generated/operation-map.json').read_text())
if generated != expected:
    print('generated operation-map.json does not match frozen map', file=sys.stderr); raise SystemExit(1)
ops=expected['operations']
for label, values in [('operation',[x['operation'] for x in ops]),('request',[x['request'] for x in ops]),('response',[x['response'] for x in ops])]:
    if len(values) != len(set(values)): raise SystemExit(f'duplicate {label}: {values}')
def members(path,name):
    text=re.sub(r'//.*','',path.read_text()); m=re.search(rf'\bunion\s+{re.escape(name)}\s*\{{(.*?)\}}',text,re.S)
    if not m: raise SystemExit(f'union {name} missing in {path}')
    return [x.strip() for x in m.group(1).split(',') if x.strip()]
req=members(root/'api/searchkernel_ffi_request.fbs','RequestPayload'); resp=members(root/'api/searchkernel_ffi_response.fbs','ResponsePayload')
want_req=[x['request'] for x in ops]; want_resp=[x['response'] for x in ops]+[expected['error_response']]
if req != want_req: raise SystemExit(f'RequestPayload mismatch\nFBS={req}\nmap={want_req}')
resp_without_error=[x for x in resp if x != expected['error_response']]
if resp.count(expected['error_response']) != 1: raise SystemExit(f'ResponsePayload must contain Error exactly once: {resp}')
if resp_without_error != [x['response'] for x in ops]: raise SystemExit(f'ResponsePayload normal variants mismatch\nFBS={resp_without_error}\nmap={[x["response"] for x in ops]}')
dispatch=(root/'crates/searchkernel-ffi/src/dispatch.rs').read_text()
for op in ops:
    token='REQUEST_'+op['operation'].upper()
    if token not in dispatch: raise SystemExit(f'dispatcher missing {token}')
client=(root/'crates/searchkernel-client/src/generated.rs').read_text()
for op in ops:
    if not re.search(rf'\bfn\s+{re.escape(op["operation"])}\s*\(',client): raise SystemExit(f'generated client missing {op["operation"]}')
print('C6 operation coverage PASS')
