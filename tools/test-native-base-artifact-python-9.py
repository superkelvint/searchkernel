#!/usr/bin/env python3
import json
import sys
for path in sys.argv[1:]:
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    assert data["artifact_generation"] == 3, (path, data)
