#!/usr/bin/env python3
import hashlib
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
manifest_rs = pathlib.Path(sys.argv[2]).read_text()
m5_data = pathlib.Path(sys.argv[3])

def constant(name: str) -> int:
    match = re.search(
        rf"pub\(crate\) const {re.escape(name)}: u32 = (\d+);",
        manifest_rs,
    )
    if not match:
        raise SystemExit(f"could not read {name} from manifest reader")
    return int(match.group(1))

supported = {
    constant("LEGACY_MANIFEST_FORMAT_VERSION"),
    constant("PREVIOUS_MANIFEST_FORMAT_VERSION"),
    constant("MANIFEST_FORMAT_VERSION"),
}

fixture_dirs = sorted(root.glob("manifest-v*-storage-v1"))
seen = set()
expected_provenance = {
    2: ("a7abfe6213dea5e7fc1132d52c1fef163b42f729", 5),
    3: ("e5b7f60cdf837866355756b55d3cb1e77d5c9b62", 6),
    4: ("e02da1cf4c91544a7dbab120e961c47d26b67278", 12),
}

def file_map(directory: pathlib.Path):
    return {
        path.relative_to(directory).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in directory.rglob("*")
        if path.is_file()
    }

m5_files = file_map(m5_data)
if not m5_files:
    raise SystemExit("preserved M5 storage fixture is empty")

for directory in fixture_dirs:
    metadata_path = directory / "METADATA.json"
    manifest_path = directory / "manifest" / "index.json"
    if not metadata_path.is_file():
        raise SystemExit(f"{directory}: missing METADATA.json")
    if not manifest_path.is_file():
        raise SystemExit(f"{directory}: missing committed manifest/index.json")
    if not (directory / "data").is_dir():
        raise SystemExit(f"{directory}: missing persisted data tree")

    metadata = json.loads(metadata_path.read_text())
    manifest = json.loads(manifest_path.read_text())
    version = metadata["manifest_version"]
    seen.add(version)

    if version not in expected_provenance:
        raise SystemExit(f"{directory}: no frozen provenance contract for manifest v{version}")
    expected_revision, expected_abi = expected_provenance[version]

    if metadata["fixture_format"] != 1:
        raise SystemExit(f"{directory}: unsupported fixture metadata format")
    if metadata["storage_compatibility_generation"] != 1:
        raise SystemExit(f"{directory}: storage generation drifted")
    if metadata["manifest_format_revision"] != expected_revision:
        raise SystemExit(f"{directory}: manifest-format revision provenance drifted")
    if metadata["manifest_native_abi_version"] != expected_abi:
        raise SystemExit(f"{directory}: manifest ABI provenance drifted")
    if metadata["vespa_revision"] != "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e":
        raise SystemExit(f"{directory}: unexpected Vespa provenance")
    if metadata["storage_searchkernel_revision"] != "7ec21655f2c5a311aa2fe4fe65861f9a8f2e3487":
        raise SystemExit(f"{directory}: M5 storage provenance drifted")
    if metadata["storage_native_abi_version"] != 4:
        raise SystemExit(f"{directory}: M5 storage ABI provenance drifted")
    if set(metadata["expected_queries"]) != {"10", "20", "30", "40"}:
        raise SystemExit(f"{directory}: expected-query corpus is incomplete")

    actual_version = manifest.get("manifest_format_version", manifest.get("format_version"))
    if actual_version != version:
        raise SystemExit(f"{directory}: manifest version disagrees with metadata")

    config_dir = directory / "config"
    required_config = {
        "searchkernel-schema.json",
        "indexschema.cfg",
        "attributes.cfg",
        "summary.cfg",
        "rank-profiles.cfg",
        "juniperrc.cfg",
        "proton.cfg",
    }
    config = {
        path.name: path.read_text()
        for path in config_dir.iterdir()
        if path.is_file()
    }
    if set(config) != required_config:
        raise SystemExit(f"{directory}: frozen config file set drifted")
    if manifest.get("resolved_engine_config_identity") != config:
        raise SystemExit(f"{directory}: frozen config identity disagrees with manifest")

    if file_map(directory / "data") != m5_files:
        raise SystemExit(f"{directory}: persisted storage bytes disagree with preserved M5 provenance")

    if version == 2:
        if "resolved_index_definition" not in manifest:
            raise SystemExit(f"{directory}: legacy resolved definition is missing")
    else:
        if manifest.get("storage_compatibility_generation") != 1:
            raise SystemExit(f"{directory}: binary manifest storage generation drifted")
        for name in ("index-definition.bfbs", "portable-index-definition.bfbs"):
            sidecar = directory / "manifest" / name
            if not sidecar.is_file() or not sidecar.read_bytes():
                raise SystemExit(f"{directory}: {name} is missing or empty")

    if version == 4:
        if manifest.get("resolved_schema_snapshot_version") != 1:
            raise SystemExit(f"{directory}: v4 snapshot version missing")
        if not isinstance(manifest.get("resolved_schema_snapshot"), dict):
            raise SystemExit(f"{directory}: v4 schema snapshot missing")

if seen != supported:
    raise SystemExit(
        f"fixture versions are {sorted(seen)}, but IndexRuntime::open claims {sorted(supported)}"
    )
