#!/usr/bin/env python3
import json
import sys

path, build_env, target, commit = sys.argv[1:]
try:
    with open(path, encoding="utf-8") as stream:
        marker = json.load(stream)
except (OSError, json.JSONDecodeError) as exc:
    raise SystemExit(f"invalid published Vespa provenance marker {path}: {exc}") from exc

expected = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa provenance {key} mismatch: "
            f"expected {value!r}, found {marker.get(key)!r}"
        )
generation = marker.get("artifact_generation")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("published Vespa provenance has no valid artifact_generation")
if marker.get("adapter_static_link_closure") is not True:
    raise SystemExit("published Vespa provenance does not certify adapter static-link closure")

format_version = marker.get("format_version")
legacy_generation_2 = format_version == 1 and generation == 2
if format_version != 2 and not legacy_generation_2:
    raise SystemExit(
        "unsupported published Vespa provenance format: "
        f"format_version={format_version!r}, artifact_generation={generation!r}"
    )
