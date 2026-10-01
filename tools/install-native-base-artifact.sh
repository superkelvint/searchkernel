#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")

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
[[ -x "$PYTHON_BIN" ]] || { echo "published Vespa base acquisition requires the repository Python" >&2; exit 1; }

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
    local repo=${SEARCHKERNEL_GITHUB_REPO:-superkelvint/searchkernel}
    local prefix="https://github.com/${repo}/releases/download/"
    local remainder tag asset release_json asset_url gh_download

    if [[ "$url" == "$prefix"* ]]; then
        remainder=${url#"$prefix"}
        tag=${remainder%%/*}
        asset=${remainder#*/}
        [[ -n "$tag" && -n "$asset" && "$asset" != "$remainder" ]] || return 1

        if [[ -n "$token" ]]; then
            release_json=$TMP/release.json
            curl -fsSL \
                -H "Authorization: Bearer $token" \
                -H "Accept: application/vnd.github+json" \
                -H "X-GitHub-Api-Version: 2022-11-28" \
                "https://api.github.com/repos/${repo}/releases/tags/$tag" \
                -o "$release_json" || return 1

            asset_url=$("$PYTHON_BIN" - "$release_json" "$asset" <<'PY'
import json
import sys

path, wanted = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    release = json.load(stream)
for candidate in release.get("assets", []):
    if candidate.get("name") == wanted:
        print(candidate["url"])
        break
else:
    raise SystemExit(f"release asset not found: {wanted}")
PY
            ) || return 1

            curl -fsSL \
                -H "Authorization: Bearer $token" \
                -H "Accept: application/octet-stream" \
                "$asset_url" \
                -o "$destination"
            return
        fi

        if command -v gh >/dev/null 2>&1; then
            gh_download="$TMP/gh-download"
            rm -rf "$gh_download"
            mkdir -p "$gh_download"
            gh release download "$tag" \
                -R "$repo" \
                -D "$gh_download" \
                -p "$asset" >/dev/null || return 1
            [[ -f "$gh_download/$asset" ]] || return 1
            mv "$gh_download/$asset" "$destination"
            return
        fi
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
"$PYTHON_BIN" - "$ARCHIVE" "$EXTRACT" <<'PY'
import pathlib
import posixpath
import sys
import tarfile

archive, destination = sys.argv[1:]
with tarfile.open(archive, "r:gz") as stream:
    members = stream.getmembers()
    if not members:
        raise SystemExit("published Vespa base archive is empty")

    by_name = {str(pathlib.PurePosixPath(member.name)): member for member in members}
    symlinks = set()
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts:
            raise SystemExit(f"unsafe published Vespa base path: {member.name}")
        if member.isdev():
            raise SystemExit(f"unsupported published Vespa base entry: {member.name}")
        if member.issym():
            target = pathlib.PurePosixPath(member.linkname)
            if target.is_absolute():
                raise SystemExit(f"unsafe published Vespa base symlink: {member.name}")
            resolved = posixpath.normpath(str(path.parent / target))
            if resolved == ".." or resolved.startswith("../"):
                raise SystemExit(f"unsafe published Vespa base symlink: {member.name}")
            symlinks.add(path)
        elif member.islnk():
            target = pathlib.PurePosixPath(member.linkname)
            if target.is_absolute() or ".." in target.parts:
                raise SystemExit(f"unsafe published Vespa base hardlink: {member.name}")
            resolved = posixpath.normpath(str(target))
            target_member = by_name.get(resolved)
            if target_member is None or not target_member.isfile():
                raise SystemExit(f"unsafe published Vespa base hardlink: {member.name}")

    # A later archive member must never be extracted through a symlink created
    # by an earlier member. Legitimate runtime SONAME links point at sibling
    # files and have no children of their own.
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        for parent in path.parents:
            if parent in symlinks:
                raise SystemExit(f"unsafe published Vespa base symlink traversal: {member.name}")

    stream.extractall(destination, members=members)
PY

BUNDLE=$EXTRACT/searchkernel-vespa-base
BUNDLE_MANIFEST=$BUNDLE/manifest.json
BASE=$BUNDLE/base
VESPA_ROOT=$BUNDLE/toolchain/upstream/vespa
DEPS=$BUNDLE/toolchain/vespa-deps
MARKER=$VESPA_ROOT/.searchkernel-vespa-artifact.json

"$PYTHON_BIN" - "$BUNDLE_MANIFEST" "$BASE/manifest.json" "$MARKER" "$BUILD_ENV" "$TARGET" "$VESPA_COMMIT" <<'PY'
import json
import sys
import pathlib
import hashlib

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
if bundle_format not in (2, 3) and not legacy_link_complete:
    raise SystemExit(
        "unsupported published Vespa base format: "
        f"format_version={bundle_format!r}, artifact_generation={generation!r}"
    )

if bundle_format == 3:
    provenance = pathlib.Path(base_path).parent / "lib" / ".searchkernel-runtime-provenance.json"
    expected_hash = bundle.get("runtime_provenance_sha256")
    marker_hash = marker.get("runtime_provenance_sha256")
    if not isinstance(expected_hash, str) or expected_hash != marker_hash:
        raise SystemExit("published runtime provenance digest is missing or inconsistent")
    try:
        actual_hash = hashlib.sha256(provenance.read_bytes()).hexdigest()
    except OSError as exc:
        raise SystemExit(f"published runtime provenance is missing: {exc}") from exc
    if actual_hash != expected_hash:
        raise SystemExit("published runtime provenance digest does not match the packaged file")

if base.get("kind") != "vespa-base" or base.get("vespa_commit") != commit:
    raise SystemExit("published Vespa base manifest does not match the pinned Vespa checkout")
PY

[[ -d "$BASE/lib" ]] || { echo "published Vespa base has no lib directory" >&2; exit 1; }
if grep -Eq '"format_version"[[:space:]]*:[[:space:]]*3' "$BUNDLE_MANIFEST"; then
    "$PYTHON_BIN" "$ROOT/tools/check-native-runtime-closure.py" \
        "$BASE/lib" --require-provenance >&2
fi
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
"$PYTHON_BIN" - "$CONFIGURED_DIR/link.txt" "$VESPA_ROOT" <<'PY'
import pathlib
import shlex
import sys

link_file = pathlib.Path(sys.argv[1]).resolve()
vespa_root = pathlib.Path(sys.argv[2]).resolve()

for token in shlex.split(link_file.read_text(encoding="utf-8")):
    if not token.endswith(".a"):
        continue
    if pathlib.Path(token).name == "libsearchcore_test.a":
        continue

    if token.startswith("/work/vespa/"):
        archive = vespa_root / token[len("/work/vespa/"):]
    else:
        archive = pathlib.Path(token)
        if not archive.is_absolute():
            # CMake evaluates link.txt arguments from the target directory,
            # one level above CMakeFiles/<target>.dir.
            archive = link_file.parent.parent.parent / archive

    archive = archive.resolve()
    try:
        archive.relative_to(vespa_root)
    except ValueError as exc:
        raise SystemExit(
            f"published Vespa link references static archive outside packaged toolchain: {token}"
        ) from exc

    if not archive.is_file():
        raise SystemExit(f"published Vespa base is missing adapter static archive: {token}")
PY

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
