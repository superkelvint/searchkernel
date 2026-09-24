#!/usr/bin/env python3
import json,sys
for c in json.load(open(sys.argv[1]))['cases']:
 for p in ('typed','yql'):
  if p in c: print(c['id'],p,c[p+'_error'])
