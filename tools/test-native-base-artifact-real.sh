#!/usr/bin/env bash
set -euo pipefail
trap 'echo "real native-base clean-room regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }
[[ -n "${VESPA_ROOT:-}" && -n "${VESPA_DEPS:-}" ]] || {
    echo "clean-room native-base test requires declared VESPA_ROOT and VESPA_DEPS inputs" >&2
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
    SEARCHKERNEL_BUILD_ENV=host \
    SK_NATIVE_BASE_GENERATION=1 \
    SK_NATIVE_TARGET="$TARGET" \
    VESPA_ROOT="$VESPA_ROOT" \
    VESPA_DEPS="$VESPA_DEPS" \
    env -u SK_NATIVE_SDK -u LD_LIBRARY_PATH -u LD_PRELOAD \
    bash "$ROOT/tools/build-native-base-release.sh" host "$archive_dir" 1 \
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
