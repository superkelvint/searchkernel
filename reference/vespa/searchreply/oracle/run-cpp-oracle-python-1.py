#!/usr/bin/env python3
import json, sys
values = {}
for line in open(sys.argv[1], encoding="utf-8"):
    key, value = line.rstrip("\n").split("=", 1)
    values[key] = value
expected = json.load(open(sys.argv[2], encoding="utf-8"))["commit"]
if values.get("vespa_commit") != expected:
    raise SystemExit("SearchReply oracle was not built from the pinned Vespa revision")
