#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) != 3:
    raise SystemExit("usage: compare_json.py EXPECTED ACTUAL")

expected_path = Path(sys.argv[1])
actual_path = Path(sys.argv[2])

with expected_path.open() as f:
    expected = json.load(f)
with actual_path.open() as f:
    actual = json.load(f)

if expected != actual:
    print("JSON mismatch", file=sys.stderr)
    print("EXPECTED:", file=sys.stderr)
    print(json.dumps(expected, indent=2, sort_keys=True), file=sys.stderr)
    print("ACTUAL:", file=sys.stderr)
    print(json.dumps(actual, indent=2, sort_keys=True), file=sys.stderr)
    raise SystemExit(1)
