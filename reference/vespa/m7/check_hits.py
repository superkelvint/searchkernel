#!/usr/bin/env python3
import json,sys
s=json.load(open(sys.argv[1])); a=json.load(open(sys.argv[2])); exp=sorted(s['expected_hits']); got=sorted(a['hits'])
if exp!=got:
 print('expected hits',exp,file=sys.stderr); print('actual hits',got,file=sys.stderr); raise SystemExit(1)
