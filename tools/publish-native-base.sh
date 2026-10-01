#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native-base publication failed at line $LINENO: $BASH_COMMAND" >&2' ERR

REPO=${SEARCHKERNEL_GITHUB_REPO:-superkelvint/searchkernel}
MODE=${1:-all}
case "$MODE" in
    ci) BUILD_ENVS=(ci) ;;
    host) BUILD_ENVS=(host) ;;
    all) BUILD_ENVS=(host ci) ;;
    *) echo "usage: $0 [ci|host|all]" >&2; exit 2 ;;
esac

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
    echo "run this from inside a SearchKernel checkout" >&2
    exit 1
}
ROOT=$(cd "$ROOT" && pwd -P)
cd "$ROOT"
for cmd in gh git sha256sum tar; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "missing required command: $cmd" >&2; exit 1; }
done
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
SOURCE_COMMIT=$(git rev-parse HEAD)
TARGET=${SK_NATIVE_TARGET:-x86_64-unknown-linux-gnu}
[[ "$TARGET" == "x86_64-unknown-linux-gnu" ]] || {
    echo "native-base publication currently supports only x86_64-unknown-linux-gnu, found $TARGET" >&2
    exit 1
}
PINNED_VESPA_COMMIT=$("$PYTHON_BIN" - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }
[[ "$PINNED_VESPA_COMMIT" =~ ^[0-9a-fA-F]{40}$ ]] || {
    echo "invalid pinned Vespa commit: $PINNED_VESPA_COMMIT" >&2
    exit 1
}

require_canonical_main() {
    local remote_main
    remote_main=$(gh api "repos/$REPO/git/ref/heads/main" --jq '.object.sha')
    [[ "$SOURCE_COMMIT" == "$remote_main" ]] || {
        echo "publisher checkout is $SOURCE_COMMIT, but canonical main is $remote_main" >&2
        exit 1
    }
    [[ -z "$(git status --porcelain --untracked-files=normal)" ]] || {
        echo "publisher checkout has uncommitted or untracked files" >&2
        exit 1
    }
}

require_canonical_main
gh api "repos/$REPO" >/dev/null

GENERATION=$(
    gh api --paginate "repos/$REPO/releases?per_page=100" --jq '.[].tag_name' |
        "$PYTHON_BIN" -c '
import re, sys
target, commit = sys.argv[1:]
pattern = re.compile(r"^vespa-base-v([0-9]+)-(?:host|ci)-" + re.escape(target) + "-" + re.escape(commit) + r"$")
values = [int(match.group(1)) for line in sys.stdin if (match := pattern.match(line.strip()))]
print(max(values, default=0) + 1)
' "$TARGET" "$PINNED_VESPA_COMMIT"
)
[[ "$GENERATION" =~ ^[1-9][0-9]*$ ]] || { echo "failed to allocate fresh native-base generation" >&2; exit 1; }

TMP=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-publish.XXXXXX")
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    bash "$ROOT/tools/remove-readonly-tree.sh" "$TMP" || {
        (( status != 0 )) || status=1
    }
    exit "$status"
}
trap cleanup EXIT

declare -A ARCHIVES CHECKSUMS TAGS ASSETS
for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    OUT=$TMP/artifacts-$BUILD_ENV
    mkdir -p "$OUT"
    env -u SK_NATIVE_SDK -u LD_LIBRARY_PATH -u LD_PRELOAD \
        SEARCHKERNEL_DEV_USE_CI_IMAGE=1 \
        SK_NATIVE_TARGET="$TARGET" \
        "$ROOT/dev" native-base build "$BUILD_ENV" "$OUT" "$GENERATION"

    ASSET="searchkernel-vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
    TAG="vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}"
    ARCHIVE=$OUT/$ASSET
    CHECKSUM=$ARCHIVE.sha256
    [[ -s "$ARCHIVE" && -s "$CHECKSUM" ]] || {
        echo "canonical builder did not produce the expected verified archive: $ARCHIVE" >&2
        exit 1
    }
    ARCHIVES[$BUILD_ENV]=$ARCHIVE
    CHECKSUMS[$BUILD_ENV]=$CHECKSUM
    TAGS[$BUILD_ENV]=$TAG
    ASSETS[$BUILD_ENV]=$ASSET
done

section() { printf '\n==> %s\n' "$*"; }
section "Revalidating canonical publication source"
require_canonical_main

section "Publication preflight"
for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG=${TAGS[$BUILD_ENV]}
    if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
        echo "immutable native-base release already exists: $TAG" >&2
        exit 1
    fi
done

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG=${TAGS[$BUILD_ENV]}
    section "Creating draft $TAG from canonical builder bytes"
    gh release create "$TAG" "${ARCHIVES[$BUILD_ENV]}" "${CHECKSUMS[$BUILD_ENV]}" \
        --repo "$REPO" --draft --target "$SOURCE_COMMIT" \
        --title "SearchKernel immutable Vespa base $TAG" \
        --notes "Automatically allocated native-base generation v${GENERATION} for SearchKernel ${SOURCE_COMMIT} and pinned Vespa ${PINNED_VESPA_COMMIT}."
    [[ "$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq '.isDraft')" == true ]] || {
        echo "$TAG became consumer-visible before remote verification" >&2
        exit 1
    }
done

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG=${TAGS[$BUILD_ENV]}
    ASSET=${ASSETS[$BUILD_ENV]}
    REMOTE=$TMP/remote-$BUILD_ENV
    mkdir -p "$REMOTE"
    section "Verifying uploaded draft bytes $TAG"
    gh release download "$TAG" --repo "$REPO" --dir "$REMOTE" \
        --pattern "$ASSET" --pattern "$ASSET.sha256"
    EXPECTED=$(awk 'NR == 1 {print $1}' "$REMOTE/$ASSET.sha256")
    ACTUAL=$(sha256sum "$REMOTE/$ASSET" | awk '{print $1}')
    LOCAL=$(sha256sum "${ARCHIVES[$BUILD_ENV]}" | awk '{print $1}')
    [[ "$EXPECTED" == "$ACTUAL" && "$LOCAL" == "$ACTUAL" ]] || {
        echo "$TAG uploaded bytes do not match the verified build artifact checksum" >&2
        exit 1
    }
    env -u SK_NATIVE_SDK -u LD_LIBRARY_PATH -u LD_PRELOAD \
        SEARCHKERNEL_DEV_USE_CI_IMAGE=1 SK_NATIVE_TARGET="$TARGET" \
        "$ROOT/dev" native-base verify \
        "$REMOTE/$ASSET" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT"
    [[ "$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq '.isDraft')" == true ]] || {
        echo "$TAG draft state changed before verification completed" >&2
        exit 1
    }
    echo "verified remote draft $TAG ($ACTUAL)"
done

section "Publishing verified native-base drafts"
require_canonical_main
for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    TAG=${TAGS[$BUILD_ENV]}
    gh release edit "$TAG" --repo "$REPO" --draft=false
    [[ "$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq '.isDraft')" == false ]] || {
        echo "$TAG did not become public after successful verification" >&2
        exit 1
    }
done

echo "Published native-base generation v${GENERATION}:"
printf '  %s\n' "${TAGS[@]}"
