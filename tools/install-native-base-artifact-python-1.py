#!/usr/bin/env python3
import json
import sys

path, wanted = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    release = json.load(stream)
for candidate in release.get("assets", []):
    if candidate.get("name") == wanted:
        print(candidate["url"])
        break
else:
    raise SystemExit(f"release asset not found: {wanted}")
