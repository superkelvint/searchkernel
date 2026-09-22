#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
FIXTURES="$ROOT/reference/persistence/fixtures"
EXPECTED_VERSIONS="2 3 4"

[[ -d "$FIXTURES" ]] || { echo "missing persistence fixture corpus" >&2; exit 1; }
[[ -f "$FIXTURES/README.md" ]] || { echo "missing persistence fixture README" >&2; exit 1; }

for version in $EXPECTED_VERSIONS; do
    dir="$FIXTURES/manifest-v$version-storage-v1"
    [[ -d "$dir/data" ]] || { echo "missing persisted data for v$version" >&2; exit 1; }
    [[ -f "$dir/manifest/index.json" ]] || { echo "missing committed manifest for v$version" >&2; exit 1; }
    [[ -f "$dir/METADATA.json" ]] || { echo "missing fixture metadata for v$version" >&2; exit 1; }
    for cfg in searchkernel-schema.json indexschema.cfg attributes.cfg summary.cfg rank-profiles.cfg juniperrc.cfg proton.cfg; do
        [[ -f "$dir/config/$cfg" ]] || { echo "missing frozen config $cfg for v$version" >&2; exit 1; }
    done
    if [[ "$version" != 2 ]]; then
        [[ -s "$dir/manifest/index-definition.bfbs" ]] || { echo "missing canonical BFBS for v$version" >&2; exit 1; }
        [[ -s "$dir/manifest/portable-index-definition.bfbs" ]] || { echo "missing portable BFBS for v$version" >&2; exit 1; }
    fi
done

python3 - "$FIXTURES" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
expected = {2, 3, 4}
seen = set()

for directory in sorted(root.glob("manifest-v*-storage-v1")):
    metadata = json.loads((directory / "METADATA.json").read_text())
    manifest = json.loads((directory / "manifest" / "index.json").read_text())
    version = metadata["manifest_version"]
    seen.add(version)

    if metadata["fixture_format"] != 1:
        raise SystemExit(f"{directory}: unsupported fixture metadata format")
    if metadata["storage_compatibility_generation"] != 1:
        raise SystemExit(f"{directory}: storage generation drifted")
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

    config = {
        path.name: path.read_text()
        for path in (directory / "config").iterdir()
        if path.is_file()
    }
    if manifest.get("resolved_engine_config_identity") != config:
        raise SystemExit(f"{directory}: frozen config identity disagrees with manifest")

    storage_files = [path for path in (directory / "data").rglob("*") if path.is_file()]
    if len(storage_files) < 40:
        raise SystemExit(f"{directory}: persisted storage tree is incomplete")

    if version == 2:
        if "resolved_index_definition" not in manifest:
            raise SystemExit(f"{directory}: legacy resolved definition is missing")
    else:
        if manifest.get("storage_compatibility_generation") != 1:
            raise SystemExit(f"{directory}: binary manifest storage generation drifted")
        for name in ("index-definition.bfbs", "portable-index-definition.bfbs"):
            if not (directory / "manifest" / name).read_bytes():
                raise SystemExit(f"{directory}: {name} is empty")

    if version == 4:
        if manifest.get("resolved_schema_snapshot_version") != 1:
            raise SystemExit(f"{directory}: v4 snapshot version missing")
        if not isinstance(manifest.get("resolved_schema_snapshot"), dict):
            raise SystemExit(f"{directory}: v4 schema snapshot missing")

if seen != expected:
    raise SystemExit(f"fixture versions are {sorted(seen)}, expected {sorted(expected)}")
PY

echo "Persistence fixture structure PASS"
