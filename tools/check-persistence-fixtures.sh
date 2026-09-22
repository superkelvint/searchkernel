#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
FIXTURES="$ROOT/reference/persistence/fixtures"
M5="$ROOT/reference/vespa/m5/fixtures/persisted-v1"
EXPECTED_VERSIONS="2 3 4"

[[ -d "$M5/index" ]] || { echo "missing immutable M5 storage fixture" >&2; exit 1; }
[[ -f "$M5/METADATA.json" ]] || { echo "missing M5 fixture provenance" >&2; exit 1; }

for version in $EXPECTED_VERSIONS; do
    dir="$FIXTURES/manifest-v$version-storage-v1"
    [[ -d "$dir" ]] || { echo "missing persistence fixture: $dir" >&2; exit 1; }
    [[ -f "$dir/METADATA.json" ]] || { echo "missing fixture metadata: $dir" >&2; exit 1; }
    [[ -f "$dir/manifest-template.json" ]] || { echo "missing manifest template: $dir" >&2; exit 1; }
    for cfg in searchkernel-schema.json indexschema.cfg attributes.cfg summary.cfg rank-profiles.cfg juniperrc.cfg proton.cfg; do
        [[ -f "$dir/config/$cfg" ]] || { echo "missing frozen config $cfg for v$version" >&2; exit 1; }
    done
    if [[ "$version" != 2 ]]; then
        [[ -f "$dir/index-definition.bfbs" ]] || { echo "missing canonical BFBS for v$version" >&2; exit 1; }
        [[ -f "$dir/portable-index-definition.bfbs" ]] || { echo "missing portable BFBS for v$version" >&2; exit 1; }
    fi
done

python3 - "$FIXTURES" "$M5/METADATA.json" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
m5 = json.loads(pathlib.Path(sys.argv[2]).read_text())
expected = {2, 3, 4}
seen = set()

for directory in sorted(root.glob("manifest-v*-storage-v1")):
    metadata = json.loads((directory / "METADATA.json").read_text())
    manifest = json.loads((directory / "manifest-template.json").read_text())
    version = metadata["manifest_version"]
    seen.add(version)
    if metadata["fixture_format"] != 1:
        raise SystemExit(f"{directory}: unsupported fixture metadata format")
    if metadata["storage_compatibility_generation"] != 1:
        raise SystemExit(f"{directory}: storage generation drifted")
    if metadata["vespa_revision"] != m5["created_by_vespa_revision"]:
        raise SystemExit(f"{directory}: Vespa provenance disagrees with M5 storage")
    if metadata["storage_fixture"] != "reference/vespa/m5/fixtures/persisted-v1/index":
        raise SystemExit(f"{directory}: storage fixture path drifted")
    if set(metadata["expected_queries"]) != {"10", "20", "30", "40"}:
        raise SystemExit(f"{directory}: expected-query corpus is incomplete")
    actual_version = manifest.get("manifest_format_version", manifest.get("format_version"))
    if actual_version != version:
        raise SystemExit(f"{directory}: manifest version disagrees with metadata")
    config = {path.name: path.read_text() for path in (directory / "config").iterdir() if path.is_file()}
    if manifest.get("resolved_engine_config_identity") != config:
        raise SystemExit(f"{directory}: frozen config identity disagrees with manifest")
    if version == 4:
        if manifest.get("resolved_schema_snapshot_version") != 1:
            raise SystemExit(f"{directory}: v4 snapshot version missing")
        if not isinstance(manifest.get("resolved_schema_snapshot"), dict):
            raise SystemExit(f"{directory}: v4 schema snapshot missing")
    if version in {3, 4}:
        if manifest.get("storage_compatibility_generation") != 1:
            raise SystemExit(f"{directory}: binary manifest storage generation drifted")
        for name in ("index-definition.bfbs", "portable-index-definition.bfbs"):
            if not (directory / name).read_bytes():
                raise SystemExit(f"{directory}: {name} is empty")

if seen != expected:
    raise SystemExit(f"fixture versions are {sorted(seen)}, expected {sorted(expected)}")
PY

echo "Persistence fixture structure PASS"
