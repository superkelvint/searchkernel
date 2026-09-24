#!/usr/bin/env python3
import json
import sys

path, build_env, target, commit = sys.argv[1:]
try:
    with open(path, encoding="utf-8") as stream:
        marker = json.load(stream)
except (OSError, json.JSONDecodeError) as exc:
    raise SystemExit(f"invalid published Vespa toolchain marker {path}: {exc}") from exc
expected = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )

# Installed cache markers may come from an older artifact schema. At this
# stage only provenance identity is authoritative; generation freshness is
# evaluated separately and newly downloaded artifacts are validated by the
# installer before publication into the cache.
