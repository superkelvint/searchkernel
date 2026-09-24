#!/usr/bin/env python3
import json
import sys

path, failpoints, sanitizer = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
manifest["hardening_failpoints"] = failpoints == "1"
manifest["sanitizer"] = sanitizer
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, indent=2, sort_keys=True)
    stream.write("\n")
