#!/usr/bin/env python3
import json
with open("core/native/vespa/manifest.json", encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
