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
    -Wl,-soname,libsearchkernel_runtime_dep.so \
    "$ROOT/tools/fixtures/runtime-launcher/dependency.c" \
    -o "$TMP/sdk/lib/libsearchkernel_runtime_dep.so"
"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_vespa.so.2 \
    "$ROOT/tools/fixtures/runtime-launcher/adapter.c" \
    -L"$TMP/sdk/lib" -lsearchkernel_runtime_dep \
    -o "$TMP/sdk/lib/libsearchkernel_vespa.so.2"
ln -s libsearchkernel_vespa.so.2 "$TMP/sdk/lib/libsearchkernel_vespa.so"

"$CC_BIN" "$ROOT/tools/fixtures/runtime-launcher/main.c" \
    -L"$TMP/sdk/lib" -Wl,-rpath-link,"$TMP/sdk/lib" -lsearchkernel_vespa \
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
"$LAUNCHER" node -e 'if (!process.env.NPM) process.exit(2); require("node:child_process").execFileSync(process.env.NPM, ["--version"])'
"$LAUNCHER" npm --version >/dev/null

if SEARCHKERNEL_RUNTIME_LIB_DIRS="$TMP/missing" "$LAUNCHER" native "$TMP/native-probe" >/dev/null 2>&1; then
    echo "native launcher accepted a missing runtime directory" >&2
    exit 1
fi

mkdir -p "$TMP/broken/lib"
cp "$TMP/sdk/lib/libsearchkernel_vespa.so.2" "$TMP/broken/lib/"
ln -s libsearchkernel_vespa.so.2 "$TMP/broken/lib/libsearchkernel_vespa.so"
set +e
broken_output=$(SEARCHKERNEL_RUNTIME_LIB_DIRS="$TMP/broken/lib" "$LAUNCHER" native "$TMP/native-probe" 2>&1)
broken_status=$?
set -e
[[ "$broken_status" -eq 127 ]] || {
    echo "native launcher returned $broken_status instead of 127 for an incomplete transitive closure" >&2
    exit 1
}
grep -F 'libsearchkernel_runtime_dep.so => not found' <<<"$broken_output" >/dev/null || {
    echo "native launcher did not diagnose the missing transitive DSO" >&2
    printf '%s\n' "$broken_output" >&2
    exit 1
}

resolver="$ROOT/tools/resolve-native-runtime-libs.sh"
if grep -F 'SK_NATIVE_ADAPTER_ONLY=1' "$resolver" >/dev/null ||
   grep -F 'core/native/vespa/build-sdk.sh' "$resolver" >/dev/null; then
    echo "native runtime resolver must reuse the exact built adapter; it must never synthesize one at exec time" >&2
    exit 1
fi

set +e
unresolved_output=$(
    env \
        -u SEARCHKERNEL_RUNTIME_LIB_DIRS \
        -u SEARCHKERNEL_VERIFIER_SDK \
        -u SEARCHKERNEL_VERIFIER_BASE_SDK \
        -u SEARCHKERNEL_SDK_LIB \
        -u SEARCHKERNEL_RUNTIME_ADAPTER_LIB \
        -u SEARCHKERNEL_RUNTIME_BASE_SDK \
        -u SK_NATIVE_SDK \
        -u LD_LIBRARY_PATH \
        -u LD_PRELOAD \
        "$LAUNCHER" native "$TMP/native-probe" 2>&1
)
unresolved_status=$?
set -e
[[ "$unresolved_status" -eq 1 ]] || {
    echo "native launcher returned $unresolved_status instead of failing closed without exact adapter identity" >&2
    printf '%s\n' "$unresolved_output" >&2
    exit 1
}
grep -F 'cannot determine exact native runtime adapter' <<<"$unresolved_output" >/dev/null || {
    echo "native launcher did not explain how to provide exact adapter identity" >&2
    printf '%s\n' "$unresolved_output" >&2
    exit 1
}

SEARCHKERNEL_RUNTIME_ADAPTER_LIB="$TMP/sdk/lib/libsearchkernel_vespa.so.2" \
SEARCHKERNEL_RUNTIME_BASE_SDK="$TMP/sdk" \
LD_LIBRARY_PATH="$TMP/broken/lib" \
    "$LAUNCHER" native "$TMP/native-probe"

SEARCHKERNEL_VERIFIER_SDK="$TMP/sdk" \
SEARCHKERNEL_VERIFIER_BASE_SDK="$TMP/sdk" \
LD_LIBRARY_PATH="$TMP/broken/lib" \
    "$LAUNCHER" native "$TMP/native-probe"

echo "canonical runtime launcher regression PASS"
