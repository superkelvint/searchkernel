#!/usr/bin/env python3
import json
import sys

provenance = {}
for line in open(sys.argv[1], encoding="utf-8"):
    key, value = line.rstrip("\n").split("=", 1)
    provenance[key] = value
expected = json.load(open(sys.argv[2], encoding="utf-8"))["commit"]
if provenance.get("vespa_commit") != expected:
    raise SystemExit("M4 C++ oracle was not built from the pinned Vespa revision")
