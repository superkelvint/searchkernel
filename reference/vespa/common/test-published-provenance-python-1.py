#!/usr/bin/env python3
import json
import sys
path, commit, closure, format_version, generation = sys.argv[1:]
data = {
    "format_version": int(format_version),
    "artifact_generation": int(generation),
    "adapter_static_link_closure": closure == "true",
    "build_environment": "verifier-regression",
    "native_target": "x86_64-unknown-linux-gnu",
    "vespa_commit": commit,
}
with open(path, "w", encoding="utf-8") as stream:
    json.dump(data, stream, sort_keys=True)
    stream.write("\n")
