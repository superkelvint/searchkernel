#!/usr/bin/env python3
import json,sys
expected=json.load(open(sys.argv[1],encoding='utf-8'))
actual=json.load(open(sys.argv[2],encoding='utf-8')) if len(sys.argv)>2 else json.load(sys.stdin)
if expected != actual:
    raise SystemExit(f'M14 mismatch\nexpected={expected!r}\nactual={actual!r}')
