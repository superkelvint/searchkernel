#!/usr/bin/env python3
import json,sys
x=json.load(open(sys.argv[1]))
print(x['id']); print(x['schema']); print(','.join(f"{f['name']}:{f['type']}" for f in x['fields'])); print(x['yql'])
