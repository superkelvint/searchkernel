#!/usr/bin/env python3
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    marker = json.load(stream)
generation = marker.get("artifact_generation")
if generation is None:
    # Migration compatibility for v1/v2 artifacts published before generation
    # became explicit metadata. Their format version matched their release
    # generation.
    generation = marker.get("format_version")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("published native-base marker has no valid artifact generation")
print(generation)
