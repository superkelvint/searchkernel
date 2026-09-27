#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
LAUNCHER="$ROOT/tools/run-runtime.sh"
CC_BIN=${CC:-cc}

[[ -x "$LAUNCHER" ]] || {
    echo "canonical runtime launcher is missing or not executable: $LAUNCHER" >&2
    exit 1
}
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

if env -u LD_LIBRARY_PATH -u LD_PRELOAD "$TMP/native-probe" >/dev/null 2>&1; then
    echo "regression fixture unexpectedly ran without an explicit native runtime path" >&2
    exit 1
fi

SEARCHKERNEL_RUNTIME_LIB_DIRS="$TMP/sdk/lib" \
    env -u LD_LIBRARY_PATH -u LD_PRELOAD "$LAUNCHER" native "$TMP/native-probe"

cat >"$TMP/fake-php" <<'SH'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
    echo "PHP fixture"
    exit 0
fi
[[ "${1:-}" == "runtime-launcher-smoke" ]]
SH
chmod +x "$TMP/fake-php"
PHP="$TMP/fake-php" "$LAUNCHER" php runtime-launcher-smoke

cat >"$TMP/fake-ruby" <<'SH'
#!/usr/bin/env bash
if [[ "${1:-}" == "--version" ]]; then
    echo "ruby fixture"
    exit 0
fi
[[ "${1:-}" == "runtime-launcher-smoke" ]]
SH
chmod +x "$TMP/fake-ruby"
RUBY="$TMP/fake-ruby" "$LAUNCHER" ruby runtime-launcher-smoke

"$LAUNCHER" python -c 'raise SystemExit(0)'
"$LAUNCHER" node -e 'process.exit(0)'
"$LAUNCHER" npm --version >/dev/null

if SEARCHKERNEL_RUNTIME_LIB_DIRS="$TMP/missing" "$LAUNCHER" native "$TMP/native-probe" >/dev/null 2>&1; then
    echo "native launcher accepted a missing runtime directory" >&2
    exit 1
fi

echo "canonical runtime launcher regression PASS"
