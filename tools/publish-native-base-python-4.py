#!/usr/bin/env python3
import json
import sys

bundle_path, marker_path, expected_format, generation, build_env, target, commit = sys.argv[1:]
expected_format = int(expected_format)
generation = int(generation)
for path in (bundle_path, marker_path):
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    expected = {
        "format_version": expected_format,
        "artifact_generation": generation,
        "build_environment": build_env,
        "native_target": target,
        "vespa_commit": commit,
        "adapter_static_link_closure": True,
    }
    for key, value in expected.items():
        if data.get(key) != value:
            raise SystemExit(
                f"published metadata mismatch in {path}: "
                f"{key} expected {value!r}, found {data.get(key)!r}"
            )
