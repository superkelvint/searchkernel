#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
MODE=${1:-ensure}

case "$MODE" in
    ensure|path) ;;
    *) echo "usage: tools/resolve-native-sdk.sh [ensure|path]" >&2; exit 2 ;;
esac

[[ -n "$TARGET" ]] || { echo "could not determine Rust target; set SK_NATIVE_TARGET" >&2; exit 1; }

configured_marker() {
    local vespa_root=$1
    [[ -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt" &&
       -f "$vespa_root/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make" ]]
}

vespa_identity() {
    local worktree=$1
    local vespa_root="$worktree/upstream/vespa"
    local top

    top=$(git -C "$vespa_root" rev-parse --show-toplevel 2>/dev/null || true)
    if [[ -n "$top" && "$(cd -- "$top" && pwd)" == "$(cd -- "$vespa_root" && pwd)" ]]; then
        git -C "$vespa_root" rev-parse HEAD
    else
        # upstream/vespa may be an ordinary tracked directory rather than its
        # own checkout. Compare that tree, not the parent worktree HEAD, so
        # unrelated Rust/API commits do not prevent safe SDK reuse.
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

validate_link_closure() {
    local sdk=$1
    local library="$sdk/lib/libsearchkernel_vespa.so"
    local sdk_lib output missing external line name path resolved

    command -v ldd >/dev/null 2>&1 || {
        echo "native SDK runtime dependency closure validation requires ldd" >&2
        return 1
    }

    sdk_lib=$(cd -- "$sdk/lib" && pwd -P)
    if ! output=$(env -u LD_LIBRARY_PATH -u LD_PRELOAD ldd "$library" 2>&1); then
        echo "native SDK '$sdk' runtime dependency closure probe failed" >&2
        printf '%s\n' "$output" >&2
        return 1
    fi

    missing=$(printf '%s\n' "$output" | sed -n '/=> not found/p')
    if [[ -n "$missing" ]]; then
        echo "native SDK '$sdk' has incomplete runtime dependency closure:" >&2
        printf '%s\n' "$missing" >&2
        return 1
    fi

    external=
    while IFS= read -r line; do
        name=$(printf '%s\n' "$line" | awk '{print $1}')
        case "$name" in
            libvespa*.so*|libLLVM*.so*|libffi.so*|libxxhash.so*|libsearchkernel_*.so*)
                path=$(printf '%s\n' "$line" | awk '$2 == "=>" {print $3}')
                [[ -n "$path" && "$path" != "not" ]] || continue
                resolved=$(readlink -f "$path" 2>/dev/null || true)
                case "$resolved" in
                    "$sdk_lib"/*) ;;
                    *) external=$(printf '%s%s => %s\n' "$external" "$name" "$path") ;;
                esac
                ;;
        esac
    done <<<"$output"

    if [[ -n "$external" ]]; then
        echo "native SDK '$sdk' resolved private runtime dependencies outside its bundled lib directory:" >&2
        printf '%s\n' "$external" >&2
        return 1
    fi
}

valid_object() {
    [[ -f "$OBJECT/include/searchkernel_vespa.h" &&
       -f "$OBJECT/manifest.json" &&
       -f "$OBJECT/native-build-fingerprint-v2.txt" &&
       -f "$OBJECT/lib/libsearchkernel_vespa.so" ]] || return 1
    grep -q "^sha256=$DIGEST$" "$OBJECT/native-build-fingerprint-v2.txt" || return 1
    validate_link_closure "$OBJECT"
}

if valid_object; then
    printf '%s\n' "$OBJECT"
    exit 0
fi

if [[ "$MODE" == path ]]; then
    echo "native SDK cache miss for fingerprint $DIGEST" >&2
    echo "run './dev native-sdk ensure' only when native verification is required" >&2
    exit 1
fi

LOCK=$CACHE_ROOT/locks/$DIGEST.lock
command -v flock >/dev/null 2>&1 || {
    echo "native SDK cache requires flock for cross-worktree build serialization" >&2
    exit 1
}
exec 9>"$LOCK"
flock -w 300 9 || {
    echo "timed out waiting for native SDK cache lock '$LOCK'" >&2
    exit 1
}

TMP=$CACHE_ROOT/objects/.tmp-$DIGEST-$
BUILD=$CACHE_ROOT/build/$DIGEST-$
cleanup() {
    rm -rf "$TMP" "$BUILD"
}
trap cleanup EXIT INT TERM

if valid_object; then
    printf '%s\n' "$OBJECT"
    exit 0
fi

if [[ -e "$OBJECT" ]]; then
    QUARANTINE_ROOT="$CACHE_ROOT/quarantine"
    mkdir -p "$QUARANTINE_ROOT"
    QUARANTINED="$QUARANTINE_ROOT/$DIGEST.$(date +%s).$BASHPID"
    echo "native SDK cache object '$OBJECT' failed validation; quarantining and rebuilding under the cache lock" >&2
    # Published objects are mode-locked read-only. Once validation has failed
    # and this digest lock is held, make only the object directory itself
    # owner-writable so it can be retired from the published namespace. Do not
    # modify any packaged contents in place.
    chmod u+w "$OBJECT"
    mv "$OBJECT" "$QUARANTINED"
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

validate_link_closure "$TMP" || {
    echo "refusing to publish native SDK with incomplete runtime dependency closure" >&2
    exit 1
}

# Cache objects are immutable after publication. Build into private staging and
# publish only into a previously absent fingerprint key; no process ever builds
# directly inside a published object.
chmod -R a-w "$TMP"
mv "$TMP" "$OBJECT"
printf '%s\n' "$OBJECT"
