#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: $0 <artifact-url> <cache-root> <build-env> <native-target> <vespa-commit>" >&2
    exit 2
fi

ARTIFACT_URL=$1
CACHE_ROOT=$2
BUILD_ENV=$3
TARGET=$4
VESPA_COMMIT=$5
OBJECT=$CACHE_ROOT/objects/$VESPA_COMMIT
TOOLCHAIN=$CACHE_ROOT/toolchains/$VESPA_COMMIT
DOWNLOAD_ROOT=$CACHE_ROOT/download
QUARANTINE_ROOT=$CACHE_ROOT/quarantine

command -v curl >/dev/null 2>&1 || { echo "published Vespa base acquisition requires curl" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "published Vespa base acquisition requires sha256sum" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "published Vespa base acquisition requires python3" >&2; exit 1; }

mkdir -p "$DOWNLOAD_ROOT" "$QUARANTINE_ROOT"
TMP=$(mktemp -d "$DOWNLOAD_ROOT/.artifact.XXXXXX")
cleanup() {
    chmod -R u+w "$TMP" 2>/dev/null || true
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

ARCHIVE=$TMP/base.tar.gz
CHECKSUM=$TMP/base.tar.gz.sha256

curl_download() {
    local url=$1
    local output=$2
    local token=${GITHUB_TOKEN:-${GH_TOKEN:-}}
    local -a args=(-fsSL)

    case "$url" in
        https://github.com/*/releases/download/*)
            if [[ -n "$token" ]]; then
                args+=(-H "Authorization: Bearer $token")
            fi
            ;;
    esac

    curl "${args[@]}" "$url" -o "$output"
}

if ! curl_download "$ARTIFACT_URL" "$ARCHIVE"; then
    echo "published Vespa base is unavailable: $ARTIFACT_URL" >&2
    exit 10
fi
if ! curl_download "$ARTIFACT_URL.sha256" "$CHECKSUM"; then
    echo "published Vespa base checksum is unavailable: $ARTIFACT_URL.sha256" >&2
    exit 1
fi

EXPECTED_SHA=$(awk 'NR == 1 { print $1 }' "$CHECKSUM")
[[ "$EXPECTED_SHA" =~ ^[0-9a-fA-F]{64}$ ]] || {
    echo "published Vespa base checksum sidecar is malformed" >&2
    exit 1
}
ACTUAL_SHA=$(sha256sum "$ARCHIVE" | awk '{print $1}')
[[ "$ACTUAL_SHA" == "$EXPECTED_SHA" ]] || {
    echo "published Vespa base checksum mismatch" >&2
    exit 1
}

EXTRACT=$TMP/extract
mkdir -p "$EXTRACT"
python3 - "$ARCHIVE" "$EXTRACT" <<'PY'
import pathlib
import sys
import tarfile

archive, destination = sys.argv[1:]
with tarfile.open(archive, "r:gz") as stream:
    members = stream.getmembers()
    if not members:
        raise SystemExit("published Vespa base archive is empty")
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts:
            raise SystemExit(f"unsafe published Vespa base path: {member.name}")
        if member.issym() or member.islnk() or member.isdev():
            raise SystemExit(f"unsupported published Vespa base entry: {member.name}")
    stream.extractall(destination, members=members)
PY

BUNDLE=$EXTRACT/searchkernel-vespa-base
BUNDLE_MANIFEST=$BUNDLE/manifest.json
BASE=$BUNDLE/base
VESPA_ROOT=$BUNDLE/toolchain/upstream/vespa
DEPS=$BUNDLE/toolchain/vespa-deps
MARKER=$VESPA_ROOT/.searchkernel-vespa-artifact.json

python3 - "$BUNDLE_MANIFEST" "$BASE/manifest.json" "$MARKER" "$BUILD_ENV" "$TARGET" "$VESPA_COMMIT" <<'PY'
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
expected = {
    "format_version": 1,
    "build_environment": build_env,
    "native_target": target,
    "vespa_commit": commit,
}
for key, value in expected.items():
    if bundle.get(key) != value:
        raise SystemExit(
            f"published Vespa base {key} mismatch: expected {value!r}, found {bundle.get(key)!r}"
        )
    if marker.get(key) != value:
        raise SystemExit(
            f"published Vespa toolchain {key} mismatch: expected {value!r}, found {marker.get(key)!r}"
        )
if base.get("kind") != "vespa-base" or base.get("vespa_commit") != commit:
    raise SystemExit("published Vespa base manifest does not match the pinned Vespa checkout")
PY

[[ -d "$BASE/lib" ]] || { echo "published Vespa base has no lib directory" >&2; exit 1; }
find "$BASE/lib" -maxdepth 1 -type f -name 'libvespa*.so*' -print -quit | grep -q . || {
    echo "published Vespa base contains no Vespa shared libraries" >&2
    exit 1
}
[[ ! -e "$BASE/lib/libsearchkernel_vespa.so" && ! -e "$BASE/include/searchkernel_vespa.h" ]] || {
    echo "published Vespa base improperly contains Search Kernel adapter artifacts" >&2
    exit 1
}
CONFIGURED_DIR=$VESPA_ROOT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir
[[ -f "$CONFIGURED_DIR/link.txt" && -f "$CONFIGURED_DIR/flags.make" ]] || {
    echo "published Vespa base is missing configured adapter compile/link metadata" >&2
    exit 1
}
[[ -d "$DEPS/include" ]] || { echo "published Vespa base is missing dependency headers" >&2; exit 1; }
mkdir -p "$DEPS/lib64"

retire_existing() {
    local path=$1
    local kind=$2
    [[ -e "$path" ]] || return 0
    chmod -R u+w "$path" 2>/dev/null || true
    mv "$path" "$QUARANTINE_ROOT/${kind}.${VESPA_COMMIT}.$(date +%s).$BASHPID"
}

mkdir -p "$CACHE_ROOT/objects" "$CACHE_ROOT/toolchains"
retire_existing "$OBJECT" object
retire_existing "$TOOLCHAIN" toolchain

mv "$BASE" "$OBJECT"
mv "$BUNDLE/toolchain" "$TOOLCHAIN"
chmod -R a-w "$OBJECT" "$TOOLCHAIN"
printf '%s\n' "$TOOLCHAIN/upstream/vespa"