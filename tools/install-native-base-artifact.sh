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

download_artifact() {
    local url=$1
    local destination=$2
    local token=${GH_TOKEN:-${GITHUB_TOKEN:-}}
    local prefix='https://github.com/superkelvint/searchkernel/releases/download/'
    local remainder tag asset release_json asset_url

    if [[ -n "$token" && "$url" == "$prefix"* ]]; then
        remainder=${url#"$prefix"}
        tag=${remainder%%/*}
        asset=${remainder#*/}
        [[ -n "$tag" && -n "$asset" && "$asset" != "$remainder" ]] || return 1

        release_json=$TMP/release.json
        curl -fsSL \
            -H "Authorization: Bearer $token" \
            -H "Accept: application/vnd.github+json" \
            -H "X-GitHub-Api-Version: 2022-11-28" \
            "https://api.github.com/repos/superkelvint/searchkernel/releases/tags/$tag" \
            -o "$release_json" || return 1

        asset_url=$(python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/install-native-base-artifact-python-1.py" "$release_json" "$asset"
        ) || return 1

        curl -fsSL \
            -H "Authorization: Bearer $token" \
            -H "Accept: application/octet-stream" \
            "$asset_url" \
            -o "$destination"
        return
    fi

    curl -fsSL "$url" -o "$destination"
}

ARCHIVE=$TMP/base.tar.gz
CHECKSUM=$TMP/base.tar.gz.sha256
if ! download_artifact "$ARTIFACT_URL" "$ARCHIVE"; then
    echo "published Vespa base is unavailable: $ARTIFACT_URL" >&2
    exit 10
fi
if ! download_artifact "$ARTIFACT_URL.sha256" "$CHECKSUM"; then
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
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/install-native-base-artifact-python-2.py" "$ARCHIVE" "$EXTRACT"

BUNDLE=$EXTRACT/searchkernel-vespa-base
BUNDLE_MANIFEST=$BUNDLE/manifest.json
BASE=$BUNDLE/base
VESPA_ROOT=$BUNDLE/toolchain/upstream/vespa
DEPS=$BUNDLE/toolchain/vespa-deps
MARKER=$VESPA_ROOT/.searchkernel-vespa-artifact.json

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/install-native-base-artifact-python-3.py" "$BUNDLE_MANIFEST" "$BASE/manifest.json" "$MARKER" "$BUILD_ENV" "$TARGET" "$VESPA_COMMIT"

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

# A published toolchain is link-complete only when every production static
# archive referenced by the harvested Vespa link command is present. This
# deliberately rejects the old v1 shape that could appear valid until the
# consumer reached the final adapter link.
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/install-native-base-artifact-python-4.py" "$CONFIGURED_DIR/link.txt" "$VESPA_ROOT"

retire_existing() {
    local path=$1
    local kind=$2
    [[ -e "$path" ]] || return 0
    chmod -R u+w "$path" 2>/dev/null || true
    mv "$path" "$QUARANTINE_ROOT/${kind}.${VESPA_COMMIT}.$(date +%s).$BASHPID"
}

mkdir -p "$CACHE_ROOT/objects" "$CACHE_ROOT/toolchains"
# These are mutable cache namespace directories, not immutable artifact
# contents. A previous interrupted install or inherited restrictive umask must
# not make atomic publication impossible.
chmod u+rwx "$CACHE_ROOT" "$CACHE_ROOT/objects" "$CACHE_ROOT/toolchains"
retire_existing "$OBJECT" object
retire_existing "$TOOLCHAIN" toolchain

# Do not rename directly out of the extracted bundle. Published base trees
# intentionally preserve read-only directory modes, so their archive parent may
# not be a valid rename source on every filesystem. Copy into writable staging
# directories under the cache, then atomically publish those staging roots.
OBJECT_STAGE=$TMP/object-stage
TOOLCHAIN_STAGE=$TMP/toolchain-stage
mkdir -p "$OBJECT_STAGE" "$TOOLCHAIN_STAGE"
cp -a "$BASE/." "$OBJECT_STAGE/"
# The object path is keyed by Vespa checkout rather than release generation.
# Preserve the published generation inside the object so Cargo can watch this
# stable file and rebuild only the local adapter when ensure atomically refreshes
# the base in place.
chmod u+rwx "$OBJECT_STAGE"
cp "$BUNDLE_MANIFEST" "$OBJECT_STAGE/.searchkernel-native-base-artifact.json"
cp -a "$BUNDLE/toolchain/." "$TOOLCHAIN_STAGE/"
# cp -a preserves the immutable source root mode onto the staging root on GNU
# coreutils. Make only those staging directory entries writable long enough for
# publication; the installed trees are sealed read-only immediately afterward.
chmod u+rwx "$OBJECT_STAGE" "$TOOLCHAIN_STAGE"
mv "$OBJECT_STAGE" "$OBJECT"
mv "$TOOLCHAIN_STAGE" "$TOOLCHAIN"
chmod -R a-w "$OBJECT" "$TOOLCHAIN"
printf '%s\n' "$TOOLCHAIN/upstream/vespa"
