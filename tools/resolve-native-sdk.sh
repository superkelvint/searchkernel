#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
MODE=${1:-ensure}

case "$MODE" in
    ensure|path|rebuild) ;;
    *) echo "usage: tools/resolve-native-sdk.sh [ensure|path|rebuild]" >&2; exit 2 ;;
esac

[[ -n "$TARGET" ]] || { echo "could not determine Rust target; set SK_NATIVE_TARGET" >&2; exit 1; }

configured_marker() {
    local vespa_root=$1
    [[ -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt" &&
       -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]]
}

vespa_identity() {
    local worktree=$1
    if git -C "$worktree/upstream/vespa" rev-parse HEAD >/dev/null 2>&1; then
        git -C "$worktree/upstream/vespa" rev-parse HEAD
    else
        git -C "$worktree" rev-parse HEAD:upstream/vespa 2>/dev/null || true
    fi
}

resolve_vespa_root() {
    if [[ -n "${VESPA_ROOT:-}" ]]; then
        configured_marker "$VESPA_ROOT" || {
            echo "VESPA_ROOT '$VESPA_ROOT' does not contain configured Vespa build outputs" >&2
            return 1
        }
        printf '%s\n' "$VESPA_ROOT"
        return
    fi

    if configured_marker "$ROOT/upstream/vespa"; then
        printf '%s\n' "$ROOT/upstream/vespa"
        return
    fi

    local wanted candidate candidate_identity
    wanted=$(vespa_identity "$ROOT")
    while IFS= read -r candidate; do
        [[ "$candidate" != "$ROOT" ]] || continue
        configured_marker "$candidate/upstream/vespa" || continue
        if [[ -n "$wanted" ]]; then
            candidate_identity=$(vespa_identity "$candidate")
            [[ "$candidate_identity" == "$wanted" ]] || continue
        fi
        printf '%s\n' "$candidate/upstream/vespa"
        return
    done < <(git -C "$ROOT" worktree list --porcelain | sed -n 's/^worktree //p')

    echo "no configured Vespa build outputs found in this or a sibling Git worktree" >&2
    echo "build/configure upstream/vespa once in any worktree, or set VESPA_ROOT" >&2
    return 1
}

COMMON_GIT_DIR=$(git -C "$ROOT" rev-parse --git-common-dir)
if [[ "$COMMON_GIT_DIR" != /* ]]; then
    COMMON_GIT_DIR=$(cd -- "$ROOT/$COMMON_GIT_DIR" && pwd)
fi
CACHE_BASE=${SK_NATIVE_CACHE_DIR:-$COMMON_GIT_DIR/searchkernel/native-sdk-cache}
CACHE_ROOT=$CACHE_BASE/$BUILD_ENV/$TARGET
mkdir -p "$CACHE_ROOT/objects" "$CACHE_ROOT/locks" "$CACHE_ROOT/build"

VESPA_BUILD_ROOT=$(resolve_vespa_root)
FINGERPRINT=$(python3 "$ROOT/tools/native-build-fingerprint.py" \
    --root "$ROOT" \
    --vespa-root "$VESPA_BUILD_ROOT" \
    --target "$TARGET")
DIGEST=$(printf '%s' "$FINGERPRINT" | sed -n 's/^sha256=//p' | head -1)
[[ ${#DIGEST} -eq 64 ]] || { echo "native fingerprint did not produce a SHA-256 digest" >&2; exit 1; }
OBJECT=$CACHE_ROOT/objects/$DIGEST

valid_object() {
    [[ -f "$OBJECT/include/searchkernel_vespa.h" &&
       -f "$OBJECT/manifest.json" &&
       -f "$OBJECT/native-build-fingerprint-v2.txt" &&
       -f "$OBJECT/lib/libsearchkernel_vespa.so" ]] || return 1
    grep -q "^sha256=$DIGEST$" "$OBJECT/native-build-fingerprint-v2.txt"
}

if [[ "$MODE" != rebuild ]] && valid_object; then
    printf '%s\n' "$OBJECT"
    exit 0
fi

LOCK=$CACHE_ROOT/locks/$DIGEST.lock
acquire_lock() {
    local attempts=0
    until mkdir "$LOCK" 2>/dev/null; do
        if (( attempts >= 300 )); then
            echo "timed out waiting for native SDK cache lock '$LOCK'" >&2
            return 1
        fi
        sleep 1
        attempts=$((attempts + 1))
        if [[ "$MODE" != rebuild ]] && valid_object; then
            printf '%s\n' "$OBJECT"
            exit 0
        fi
    done
}

acquire_lock
TMP=$CACHE_ROOT/objects/.tmp-$DIGEST-$$
BUILD=$CACHE_ROOT/build/$DIGEST-$$
cleanup() {
    rm -rf "$TMP" "$BUILD" "$LOCK"
}
trap cleanup EXIT INT TERM

if [[ "$MODE" != rebuild ]] && valid_object; then
    printf '%s\n' "$OBJECT"
    exit 0
fi

rm -rf "$TMP" "$BUILD"
BUILDER=${SK_NATIVE_CACHE_BUILDER:-$ROOT/core/native/vespa/build-sdk.sh}
VESPA_ROOT="$VESPA_BUILD_ROOT" \
SK_NATIVE_SDK="$TMP" \
SK_NATIVE_BUILD_DIR="$BUILD" \
SK_NATIVE_TARGET="$TARGET" \
SK_NATIVE_REGENERATE_BINDINGS=0 \
SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
    "$BUILDER"

grep -q "^sha256=$DIGEST$" "$TMP/native-build-fingerprint-v2.txt" || {
    echo "built SDK fingerprint does not match requested cache key $DIGEST" >&2
    exit 1
}

# Cache objects are immutable after publication. A rebuild creates a complete
# replacement in a private staging directory and swaps it into place atomically;
# no process ever builds directly inside a published object.
chmod -R a-w "$TMP"
if [[ -e "$OBJECT" ]]; then
    OLD=$CACHE_ROOT/objects/.old-$DIGEST-$$
    mv "$OBJECT" "$OLD"
    mv "$TMP" "$OBJECT"
    rm -rf "$OLD"
else
    mv "$TMP" "$OBJECT"
fi

printf '%s\n' "$OBJECT"
