#!/usr/bin/env python3
import json
import sys

actual = json.loads(sys.argv[1])
expected = json.load(open(sys.argv[2], encoding="utf-8"))
if actual != expected:
    raise SystemExit(f"phase9 native probe mismatch: {actual!r} != {expected!r}")
