#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) != 3:
    raise SystemExit('usage: compare.py EXPECTED ACTUAL')
expected = json.loads(Path(sys.argv[1]).read_text())
actual = json.loads(Path(sys.argv[2]).read_text())
if expected != actual:
    print('M6 semantic configuration mismatch', file=sys.stderr)
    print('EXPECTED:', json.dumps(expected, indent=2, sort_keys=True), file=sys.stderr)
    print('ACTUAL:  ', json.dumps(actual, indent=2, sort_keys=True), file=sys.stderr)
    raise SystemExit(1)
