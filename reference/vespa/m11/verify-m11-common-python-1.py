#!/usr/bin/env python3
import json
import sys
manifest = json.load(open(sys.argv[1], encoding="utf-8"))
if not manifest.get("hardening_failpoints", False):
    raise SystemExit("SDK is not a hardening/failpoint SDK")
