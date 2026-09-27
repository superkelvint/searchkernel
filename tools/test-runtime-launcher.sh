#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
LAUNCHER="$ROOT/tools/run-runtime.sh"
CC_BIN=${CC:-cc}

command -v "$CC_BIN" >/dev/null 2>&1 || {
    echo "runtime launcher regression requires a C compiler" >&2
    exit 1
}

TMP=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-runtime-launcher.XXXXXX")
cleanup() {
    rm -rf -- "$TMP"
}
trap cleanup EXIT INT TERM

mkdir -p "$TMP/sdk/lib"
"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_vespa.so.2 \
    "$ROOT/tools/fixtures/runtime-launcher/adapter.c" \
    -o "$TMP/sdk/lib/libsearchkernel_vespa.so.2"
ln -s libsearchkernel_vespa.so.2 "$TMP/sdk/lib/libsearchkernel_vespa.so"

"$CC_BIN" "$ROOT/tools/fixtures/runtime-launcher/main.c" \
    -L"$TMP/sdk/lib" -lsearchkernel_vespa \
    -o "$TMP/native-probe"

if env -u LD_LIBRARY_PATH "$TMP/native-probe" >/dev/null 2>&1; then
    echo "regression fixture unexpectedly ran without an explicit native runtime path" >&2
    exit 1
fi

SEARCHKERNEL_RUNTIME_LIB_DIRS="$TMP/sdk/lib" \
    env -u LD_LIBRARY_PATH "$LAUNCHER" native "$TMP/native-probe"

echo "canonical runtime launcher regression PASS"
