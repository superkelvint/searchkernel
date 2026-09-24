#!/usr/bin/env python3
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as stream:
    manifest = json.load(stream)
if manifest.get("hardening_failpoints") is not True:
    raise SystemExit(
        f"hardening SDK manifest does not enable failpoints: {path}"
    )
