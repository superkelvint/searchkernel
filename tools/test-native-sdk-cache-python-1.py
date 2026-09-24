#!/usr/bin/env python3
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
