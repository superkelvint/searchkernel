#!/usr/bin/env python3
import json
import sys

metadata = json.load(open(sys.argv[1]))
manifest = json.load(open(sys.argv[2]))
if metadata["created_by_native_abi"] != 4:
    raise SystemExit("fixture provenance was rewritten")
if metadata["created_by_vespa_revision"] != manifest["vespa_commit"]:
    raise SystemExit("fixture/Vespa revision metadata mismatch")
if metadata["target"] != "linux-x86_64":
    raise SystemExit("fixture target is not explicitly recorded")
