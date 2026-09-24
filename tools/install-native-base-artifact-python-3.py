#!/usr/bin/env python3
import json
import sys

bundle_path, base_path, marker_path, build_env, target, commit = sys.argv[1:]

def read(path):
    try:
        with open(path, encoding="utf-8") as stream:
            return json.load(stream)
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"invalid published Vespa base metadata {path}: {exc}") from exc

bundle = read(bundle_path)
base = read(base_path)
marker = read(marker_path)

expected_identity = {
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected_identity.items():
    if bundle.get(key) != value:
        raise SystemExit(
            f"published Vespa base {key} mismatch: expected {value!r}, found {bundle.get(key)!r}"
        )
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )

generation = bundle.get("artifact_generation")
if not isinstance(generation, int) or generation < 1:
    raise SystemExit("published Vespa base has no valid artifact_generation")
if marker.get("artifact_generation") != generation:
    raise SystemExit(
        "published Vespa base/toolchain artifact_generation mismatch: "
        f"{generation!r} vs {marker.get('artifact_generation')!r}"
    )

bundle_format = bundle.get("format_version")
marker_format = marker.get("format_version")
if bundle_format != marker_format:
    raise SystemExit(
        "published Vespa base/toolchain format_version mismatch: "
        f"{bundle_format!r} vs {marker_format!r}"
    )

legacy_link_complete = (
    bundle_format == 1
    and generation == 2
    and bundle.get("adapter_static_link_closure") is True
    and marker.get("adapter_static_link_closure") is True
)
if bundle_format != 2 and not legacy_link_complete:
    raise SystemExit(
        "unsupported published Vespa base format: "
        f"format_version={bundle_format!r}, artifact_generation={generation!r}"
    )

if base.get("kind") != "vespa-base" or base.get("vespa_commit") != commit:
    raise SystemExit("published Vespa base manifest does not match the pinned Vespa checkout")
