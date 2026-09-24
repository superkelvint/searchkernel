#!/usr/bin/env python3
import json, sys
bundle, marker, expected_format, generation, env, target, commit = sys.argv[1:]
expected_format = int(expected_format)
generation = int(generation)
for path in (bundle, marker):
    with open(path, encoding="utf-8") as stream:
        data = json.load(stream)
    if data.get("format_version") != expected_format:
        raise SystemExit(
            f"refusing to publish stale native-base schema from {path}: "
            f"expected format_version {expected_format}, found {data.get('format_version')!r}"
        )
    if data.get("build_environment") != env or data.get("native_target") != target or data.get("vespa_commit") != commit:
        raise SystemExit(f"artifact metadata identity mismatch in {path}")
    data["artifact_generation"] = generation
    data["adapter_static_link_closure"] = True
    with open(path, "w", encoding="utf-8") as stream:
        json.dump(data, stream, indent=2, sort_keys=True)
        stream.write("\n")
