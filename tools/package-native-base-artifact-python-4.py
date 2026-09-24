#!/usr/bin/env python3
import json
import sys
bundle_path, marker_path, build_env, target, commit = sys.argv[1:]
data = {
    "format_version": 2,
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for path in (bundle_path, marker_path):
    with open(path, "w", encoding="utf-8") as stream:
        json.dump(data, stream, indent=2, sort_keys=True)
        stream.write("\n")
