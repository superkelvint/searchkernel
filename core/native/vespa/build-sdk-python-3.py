#!/usr/bin/env python3
import json
import sys

source_path, output_path = sys.argv[1:]
with open(source_path, encoding="utf-8") as stream:
    source = json.load(stream)
with open(output_path, "w", encoding="utf-8") as stream:
    json.dump(
        {"kind": "vespa-base", "vespa_commit": source["vespa_commit"]},
        stream,
        indent=2,
        sort_keys=True,
    )
    stream.write("\n")
