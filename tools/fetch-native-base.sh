#!/usr/bin/env bash
set -euo pipefail

REPO="${SEARCHKERNEL_GITHUB_REPO:-superkelvint/searchkernel}"
BUILD_ENV="${1:-ci}"

case "$BUILD_ENV" in
    ci|host) ;;
    *) echo "usage: $0 [ci|host]" >&2; exit 2 ;;
esac

die() { echo "ERROR: $*" >&2; exit 1; }

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" ||
    die "run this from inside a SearchKernel checkout"
ROOT="$(cd "$ROOT" && pwd -P)"
cd "$ROOT"

for cmd in gh python3 sha256sum; do
    command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

gh api "repos/$REPO" >/dev/null 2>&1 ||
    die "gh authentication cannot access $REPO"

PINNED_VESPA_COMMIT="$(python3 - <<'PY'
import json
with open("core/native/vespa/manifest.json", encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)"
if [[ -n "${SK_NATIVE_TARGET:-}" ]]; then
    TARGET=$SK_NATIVE_TARGET
else
    command -v rustc >/dev/null 2>&1 || die "rustc is required unless SK_NATIVE_TARGET is set"
    TARGET="$(rustc -vV | sed -n 's/^host: //p')"
fi
[[ -n "$TARGET" ]] || die "could not determine Rust target"

GENERATION="$(
    gh api --paginate "repos/${REPO}/releases?per_page=100" --jq '.[].tag_name' |
    python3 -c '
import re
import sys

build_env, target, commit = sys.argv[1:]
pattern = re.compile(
    r"^vespa-base-v([0-9]+)-"
    + re.escape(build_env)
    + "-"
    + re.escape(target)
    + "-"
    + re.escape(commit)
    + r"$"
)
versions = []
for raw in sys.stdin:
    match = pattern.match(raw.strip())
    if match:
        versions.append(int(match.group(1)))
if not versions:
    raise SystemExit(1)
print(max(versions))
' "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT"
)" || die "no published native-base generation exists for $BUILD_ENV/$TARGET/$PINNED_VESPA_COMMIT"

TAG="vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}"
ASSET="searchkernel-vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"

CACHE_DIR="${SEARCHKERNEL_ARTIFACT_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/searchkernel/native-artifacts}"
mkdir -p "$CACHE_DIR"
CACHE_DIR="$(cd "$CACHE_DIR" && pwd -P)"

ARCHIVE="$CACHE_DIR/$ASSET"
CHECKSUM="$ARCHIVE.sha256"

valid_cached_artifact() {
    [[ -s "$ARCHIVE" && -s "$CHECKSUM" ]] || return 1
    local expected actual
    expected="$(awk 'NR == 1 {print $1}' "$CHECKSUM")"
    [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || return 1
    actual="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
    [[ "$actual" == "$expected" ]]
}

if ! valid_cached_artifact; then
    tmp="$(mktemp -d "$CACHE_DIR/.native-base.XXXXXX")"
    cleanup() { rm -rf "$tmp"; }
    trap cleanup EXIT INT TERM

    echo "Downloading $TAG" >&2
    gh release download "$TAG"         -R "$REPO"         -D "$tmp"         -p "$ASSET"         -p "$ASSET.sha256"

    [[ -s "$tmp/$ASSET" && -s "$tmp/$ASSET.sha256" ]] ||
        die "release $TAG is missing archive or checksum"

    expected="$(awk 'NR == 1 {print $1}' "$tmp/$ASSET.sha256")"
    actual="$(sha256sum "$tmp/$ASSET" | awk '{print $1}')"
    [[ "$expected" == "$actual" ]] ||
        die "downloaded native-base checksum mismatch for $TAG"

    mv -f "$tmp/$ASSET" "$ARCHIVE"
    mv -f "$tmp/$ASSET.sha256" "$CHECKSUM"
    trap - EXIT INT TERM
    rm -rf "$tmp"
else
    echo "Reusing cached $TAG" >&2
fi

printf '%s\n' "$ARCHIVE"
