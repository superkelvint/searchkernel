#!/usr/bin/env bash
set -euo pipefail
trap 'echo "real native-base clean-room regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-ci}
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }
if [[ -z "${VESPA_ROOT:-}" || -z "${VESPA_DEPS:-}" ]]; then
    PINNED_VESPA_COMMIT=$(
        "$PYTHON_BIN" - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
    )
    CANONICAL_CACHE=/opt/searchkernel/native-sdk-cache/$BUILD_ENV/$TARGET
    VESPA_ROOT=$CANONICAL_CACHE/toolchains/$PINNED_VESPA_COMMIT/upstream/vespa
    VESPA_DEPS=$CANONICAL_CACHE/toolchains/$PINNED_VESPA_COMMIT/vespa-deps
fi
[[ -f "$VESPA_ROOT/.searchkernel-vespa-artifact.json" || -d "$VESPA_ROOT/.git" ]] || {
    echo "clean-room native-base test has no verified Vespa producer at $VESPA_ROOT" >&2
    exit 1
}
[[ -d "$VESPA_DEPS/include" ]] || {
    echo "clean-room native-base test has no dependency producer at $VESPA_DEPS" >&2
    exit 1
}
[[ ! -e "$ROOT/core/native/sdk" ]] || {
    echo "clean-room native-base test requires no repo-local core/native/sdk" >&2
    exit 1
}

fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-clean-room.XXXXXX")
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
    exit "$status"
}
trap cleanup EXIT

# This stub makes any GitHub operation visible and prevents a build-only test
# from accidentally mutating the release control plane.
mkdir -p "$fixture/bin"
cat >"$fixture/bin/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$GH_CALL_LOG"
exit 97
SH
chmod +x "$fixture/bin/gh"
export GH_CALL_LOG="$fixture/gh.log"

archive_dir=$fixture/artifacts
PATH="$fixture/bin:$PATH" \
CXX=/bin/false CC=/bin/false \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_BASE_GENERATION=1 \
    SK_NATIVE_TARGET="$TARGET" \
    VESPA_ROOT="$VESPA_ROOT" \
    VESPA_DEPS="$VESPA_DEPS" \
    env -u SK_NATIVE_SDK -u LD_LIBRARY_PATH -u LD_PRELOAD \
    bash "$ROOT/tools/build-native-base-release.sh" "$BUILD_ENV" "$archive_dir" 1 \
    >"$fixture/build.out"

archive=$(find "$archive_dir" -maxdepth 1 -type f -name '*.tar.gz' -print -quit)
[[ -s "$archive" && -s "$archive.sha256" ]] || {
    echo "build-only command did not produce an archive and checksum" >&2
    exit 1
}
expected=$(awk 'NR == 1 {print $1}' "$archive.sha256")
actual=$(sha256sum "$archive" | awk '{print $1}')
[[ "$expected" == "$actual" ]] || { echo "build-only checksum does not match archive bytes" >&2; exit 1; }
[[ ! -s "$GH_CALL_LOG" ]] || {
    echo "build-only path called GitHub: $(cat "$GH_CALL_LOG")" >&2
    exit 1
}

echo "native-base clean-room producer/archive consumer/runtime smoke: PASS ($actual)"
