#!/usr/bin/env python3
import json,sys
if len(sys.argv)!=3: raise SystemExit("usage: compare.py EXPECTED ACTUAL")
a=json.load(open(sys.argv[1],encoding="utf-8")); b=json.load(open(sys.argv[2],encoding="utf-8"))
if a!=b:
    print("EXPECTED:",json.dumps(a,indent=2,sort_keys=True),file=sys.stderr)
    print("ACTUAL:",json.dumps(b,indent=2,sort_keys=True),file=sys.stderr)
    raise SystemExit(1)
