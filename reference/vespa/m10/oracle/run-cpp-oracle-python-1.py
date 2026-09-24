#!/usr/bin/env python3
import json,sys
p={}
for line in open(sys.argv[1],encoding='utf-8'):
 k,v=line.rstrip('\n').split('=',1); p[k]=v
expected=json.load(open(sys.argv[2],encoding='utf-8'))['commit']
if p.get('vespa_commit')!=expected: raise SystemExit('M10 oracle not built from pinned Vespa revision')
