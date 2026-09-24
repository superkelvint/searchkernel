#!/usr/bin/env bash
set -euo pipefail

REPO="${SEARCHKERNEL_GITHUB_REPO:-superkelvint/searchkernel}"
MODE="${1:-ci}"
case "$MODE" in
    ci) BUILD_ENVS=(ci) ;;
    host) BUILD_ENVS=(host) ;;
    all) BUILD_ENVS=(host ci) ;;
    *) echo "usage: $0 [ci|host|all]" >&2; exit 2 ;;
esac

die() { echo "ERROR: $*" >&2; exit 1; }
section() { echo; echo "==> $*"; }

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || die "run this from inside a SearchKernel checkout"
ROOT="$(cd "$ROOT" && pwd -P)"
cd "$ROOT"
for cmd in gh git python3 rustc sha256sum tar ldd; do command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"; done
[[ -x ./dev ]] || die "./dev is missing or not executable"
gh api "repos/$REPO" >/dev/null 2>&1 || die "gh authentication cannot access $REPO"

PINNED_VESPA_COMMIT="$(python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/repair-native-base-python-1.py"
)"
TARGET="${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
MARKER_REL="searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt"
FLAGS_REL="searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/flags.make"

candidate_valid() {
    local candidate="$1" top actual
    [[ -d "$candidate" && -f "$candidate/CMakeCache.txt" && -f "$candidate/$MARKER_REL" && -f "$candidate/$FLAGS_REL" ]] || return 1
    top="$(git -C "$candidate" rev-parse --show-toplevel 2>/dev/null || true)"
    [[ -n "$top" && "$(cd "$top" && pwd -P)" == "$(cd "$candidate" && pwd -P)" ]] || return 1
    actual="$(git -C "$candidate" rev-parse HEAD)"
    [[ "$actual" == "$PINNED_VESPA_COMMIT" ]] || return 1
    git -C "$candidate" diff --quiet --ignore-submodules -- || return 1
    git -C "$candidate" diff --cached --quiet --ignore-submodules -- || return 1
}

REAL_VESPA_ROOT=""
if [[ -n "${VESPA_ROOT:-}" ]] && candidate_valid "$VESPA_ROOT"; then
    REAL_VESPA_ROOT="$(cd "$VESPA_ROOT" && pwd -P)"
elif candidate_valid "$ROOT/upstream/vespa"; then
    REAL_VESPA_ROOT="$(cd "$ROOT/upstream/vespa" && pwd -P)"
else
    while IFS= read -r cache; do
        candidate="$(dirname "$cache")"
        case "$candidate" in */upstream/vespa) ;; *) continue ;; esac
        if candidate_valid "$candidate"; then REAL_VESPA_ROOT="$(cd "$candidate" && pwd -P)"; break; fi
    done < <(find "$HOME" -maxdepth 8 -type f -name CMakeCache.txt -path '*/upstream/vespa/CMakeCache.txt' -print 2>/dev/null)
fi
[[ -n "$REAL_VESPA_ROOT" ]] || die "no clean configured standalone Vespa checkout found"
REAL_LINK_FILE="$REAL_VESPA_ROOT/$MARKER_REL"
REAL_FLAGS_FILE="$REAL_VESPA_ROOT/$FLAGS_REL"
RAW_INCLUDES="$(sed -n 's/^CXX_INCLUDES = //p' "$REAL_FLAGS_FILE")"
CONFIGURED_DEPS="$(printf '%s\n' "$RAW_INCLUDES" | sed -n 's#.*-isystem \([^ ]*\)/include.*#\1#p')"
WORKTREE_ROOT=""
case "$REAL_VESPA_ROOT" in */upstream/vespa) WORKTREE_ROOT="$(cd "$REAL_VESPA_ROOT/../.." && pwd -P)" ;; esac
if [[ -n "${VESPA_DEPS:-}" ]]; then REAL_DEPS="$VESPA_DEPS";
elif [[ -n "$WORKTREE_ROOT" && -d "$WORKTREE_ROOT/vespa-deps" ]]; then REAL_DEPS="$WORKTREE_ROOT/vespa-deps";
elif [[ -d "$ROOT/vespa-deps" ]]; then REAL_DEPS="$ROOT/vespa-deps";
else REAL_DEPS="$CONFIGURED_DEPS"; fi
[[ -n "$REAL_DEPS" && -d "$REAL_DEPS/include" ]] || die "Vespa dependencies unavailable"
REAL_DEPS="$(cd "$REAL_DEPS" && pwd -P)"

TMP="$(mktemp -d /tmp/searchkernel-native-base-repair.XXXXXX)"
cleanup() { [[ "${KEEP_NATIVE_BASE_TMP:-0}" == 1 ]] && echo "Keeping $TMP" || rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

latest_generation() {
    local env="$1"
    gh api --paginate "repos/${REPO}/releases?per_page=100" --jq '.[].tag_name' |
    python3 -c '
import re, sys
env, target, commit = sys.argv[1:]
pat = re.compile(r"^vespa-base-v([0-9]+)-" + re.escape(env) + "-" + re.escape(target) + "-" + re.escape(commit) + r"$")
versions=[]
for raw in sys.stdin:
    m=pat.match(raw.strip())
    if m: versions.append(int(m.group(1)))
if not versions: raise SystemExit(1)
print(max(versions))
' "$env" "$TARGET" "$PINNED_VESPA_COMMIT"
}

for BUILD_ENV in "${BUILD_ENVS[@]}"; do
    GENERATION="$(latest_generation "$BUILD_ENV")" || die "no existing native-base release for $BUILD_ENV"
    TAG="vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}"
    ASSET="searchkernel-vespa-base-v${GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
    section "Repairing $TAG"
    export SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" SK_NATIVE_TARGET="$TARGET" VESPA_ROOT="$REAL_VESPA_ROOT" VESPA_DEPS="$REAL_DEPS"
    SK_NATIVE_DISABLE_PUBLISHED_BASE=1 ./dev native-sdk ensure
    OUT="$TMP/original-$BUILD_ENV"
    mkdir -p "$OUT"
    mapfile -t FILES < <(bash tools/package-native-base-artifact.sh "$OUT")
    [[ ${#FILES[@]} -ge 2 && -s "${FILES[0]}" ]] || die "canonical packager did not emit an archive"
    STAGE="$TMP/stage-$BUILD_ENV"
    mkdir -p "$STAGE"
    tar -xzf "${FILES[0]}" -C "$STAGE"
    BUNDLE="$STAGE/searchkernel-vespa-base"
    PACKAGED_VESPA="$BUNDLE/toolchain/upstream/vespa"
    PACKAGED_DEPS="$BUNDLE/toolchain/vespa-deps"
    python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/repair-native-base-python-2.py" "$REAL_LINK_FILE" "$REAL_VESPA_ROOT" "$REAL_DEPS" "$PACKAGED_VESPA" "$PACKAGED_DEPS"
    python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/repair-native-base-python-3.py" "$BUNDLE/manifest.json" "$PACKAGED_VESPA/.searchkernel-vespa-artifact.json" "$GENERATION"
    REPAIRED="$TMP/$ASSET"
    (cd "$STAGE" && tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner -czf "$REPAIRED" searchkernel-vespa-base)
    (cd "$TMP" && sha256sum "$ASSET" > "$ASSET.sha256")
    VERIFY="$TMP/verify-$BUILD_ENV"
    mkdir -p "$VERIFY"
    tar -xzf "$REPAIRED" -C "$VERIFY"
    BASE="$VERIFY/searchkernel-vespa-base/base"; VESPA="$VERIFY/searchkernel-vespa-base/toolchain/upstream/vespa"; DEPS="$VERIFY/searchkernel-vespa-base/toolchain/vespa-deps"
    mkdir -p "$DEPS/lib64"
    env -u LD_PRELOAD SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" SK_NATIVE_TARGET="$TARGET" SK_NATIVE_ADAPTER_ONLY=1 SK_NATIVE_BASE_SDK="$BASE" SK_NATIVE_SDK="$VERIFY/sdk" SK_NATIVE_BUILD_DIR="$VERIFY/build" SK_NATIVE_REGENERATE_BINDINGS=0 SK_NATIVE_SKIP_BINDINGS_CHECK=1 VESPA_ROOT="$VESPA" VESPA_DEPS="$DEPS" VESPA_DEPS_INCLUDE_DIR="$DEPS/include" VESPA_DEPS_LIB_DIR="$DEPS/lib64" VESPA_EXTRA_LIB_DIR="$BASE/lib" bash core/native/vespa/build-sdk.sh
    [[ -s "$VERIFY/sdk/lib/libsearchkernel_vespa.so" ]] || die "adapter verification failed"
    LDD="$(env -u LD_PRELOAD LD_LIBRARY_PATH="$BASE/lib" ldd "$VERIFY/sdk/lib/libsearchkernel_vespa.so")"
    ! printf '%s\n' "$LDD" | grep -q 'not found' || die "runtime closure verification failed"
    gh release upload "$TAG" "$REPAIRED" "$TMP/$ASSET.sha256" --clobber -R "$REPO"
    REMOTE="$TMP/remote-$BUILD_ENV"; mkdir -p "$REMOTE"
    gh release download "$TAG" -R "$REPO" -D "$REMOTE" -p "$ASSET" -p "$ASSET.sha256"
    EXPECTED="$(awk 'NR==1{print $1}' "$REMOTE/$ASSET.sha256")"; ACTUAL="$(sha256sum "$REMOTE/$ASSET" | awk '{print $1}')"
    [[ "$EXPECTED" == "$ACTUAL" ]] || die "uploaded checksum mismatch for $TAG"
    echo "Repaired $TAG ($ACTUAL)"
done
