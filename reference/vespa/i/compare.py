#!/usr/bin/env python3
import json
import sys

if len(sys.argv) != 3:
    raise SystemExit("usage: compare.py expected actual")
with open(sys.argv[1], encoding="utf-8") as stream:
    expected = json.load(stream)
with open(sys.argv[2], encoding="utf-8") as stream:
    actual = json.load(stream)
if expected != actual:
    print("I-series product-path output mismatch", file=sys.stderr)
    print(json.dumps(expected, indent=2, sort_keys=True), file=sys.stderr)
    print(json.dumps(actual, indent=2, sort_keys=True), file=sys.stderr)
    raise SystemExit(1)
print("I-series product-path output matches frozen expected result")
