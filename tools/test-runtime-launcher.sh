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
        -u NATIVE_ADAPTER_LIB \
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

# Reproduce the Cargo-specific identity problem: the executable was linked
# against a private adapter under Cargo OUT_DIR, while an incompatible adapter
# with the same SONAME is ambient at exec time. The launcher must recover the
# exact link-time adapter from Cargo build metadata, not guess or rebuild one.
cargo_target="$TMP/cargo-target"
cargo_out="$cargo_target/debug/build/searchkernel-vespa-bindings-fixture/out"
cargo_adapter="$cargo_out/native-adapter/lib"
cargo_base="$TMP/cargo-base"
wrong_sdk="$TMP/wrong-sdk"
mkdir -p "$cargo_adapter" "$cargo_base/lib" "$wrong_sdk/lib"

"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_runtime_dep.so \
    "$ROOT/tools/fixtures/runtime-launcher/dependency.c" \
    -o "$cargo_base/lib/libsearchkernel_runtime_dep.so"
"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_vespa.so.2 \
    "$ROOT/tools/fixtures/runtime-launcher/adapter.c" \
    -L"$cargo_base/lib" -lsearchkernel_runtime_dep \
    -o "$cargo_adapter/libsearchkernel_vespa.so.2"
ln -s libsearchkernel_vespa.so.2 "$cargo_adapter/libsearchkernel_vespa.so"

sed 's/return 42;/return 7;/' \
    "$ROOT/tools/fixtures/runtime-launcher/dependency.c" >"$TMP/wrong-dependency.c"
"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_runtime_dep.so \
    "$TMP/wrong-dependency.c" \
    -o "$wrong_sdk/lib/libsearchkernel_runtime_dep.so"
"$CC_BIN" -shared -fPIC \
    -Wl,-soname,libsearchkernel_vespa.so.2 \
    "$ROOT/tools/fixtures/runtime-launcher/adapter.c" \
    -L"$wrong_sdk/lib" -lsearchkernel_runtime_dep \
    -o "$wrong_sdk/lib/libsearchkernel_vespa.so.2"
ln -s libsearchkernel_vespa.so.2 "$wrong_sdk/lib/libsearchkernel_vespa.so"

"$CC_BIN" "$ROOT/tools/fixtures/runtime-launcher/main.c" \
    -L"$cargo_adapter" \
    -Wl,-rpath-link,"$cargo_adapter" \
    -Wl,-rpath-link,"$cargo_base/lib" \
    -lsearchkernel_vespa \
    -o "$TMP/cargo-native-probe"

set +e
LD_LIBRARY_PATH="$wrong_sdk/lib" "$TMP/cargo-native-probe" >/dev/null 2>&1
wrong_status=$?
set -e
[[ "$wrong_status" -eq 1 ]] || {
    echo "Cargo runtime fixture did not demonstrate adapter semantic drift" >&2
    exit 1
}

cargo_messages="$TMP/cargo-messages.jsonl"
printf '{"reason":"build-script-executed","package_id":"path+file:///work/core/crates/searchkernel-vespa-bindings#0.1.0","linked_paths":["native=%s"],"out_dir":"%s"}\n' \
    "$cargo_adapter" "$cargo_out" >"$cargo_messages"

env \
    -u SEARCHKERNEL_RUNTIME_LIB_DIRS \
    -u SEARCHKERNEL_VERIFIER_SDK \
    -u SEARCHKERNEL_VERIFIER_BASE_SDK \
    -u SEARCHKERNEL_SDK_LIB \
    -u SEARCHKERNEL_RUNTIME_ADAPTER_LIB \
    -u NATIVE_ADAPTER_LIB \
    -u SK_NATIVE_SDK \
    SEARCHKERNEL_CARGO_MESSAGES="$cargo_messages" \
    SEARCHKERNEL_RUNTIME_BASE_SDK="$cargo_base" \
    CARGO_TARGET_DIR="$cargo_target" \
    LD_LIBRARY_PATH="$wrong_sdk/lib" \
    "$LAUNCHER" native "$TMP/cargo-native-probe"

searchreply_verifier="$ROOT/reference/vespa/searchreply/verify-searchreply.sh"
grep -F 'SEARCHKERNEL_CARGO_MESSAGES=' "$searchreply_verifier" >/dev/null || {
    echo "SearchReply verifier does not preserve Cargo adapter identity for runtime launch" >&2
    exit 1
}
grep -F 'tools/run-runtime.sh" native "$CONFIG_BIN"' "$searchreply_verifier" >/dev/null || {
    echo "SearchReply verifier bypasses the canonical native runtime launcher" >&2
    exit 1
}
if grep -Eq '^[[:space:]]*"\$CONFIG_BIN"[[:space:]]+"\$TMP/config"' "$searchreply_verifier"; then
    echo "SearchReply verifier directly executes its Cargo-built native binary" >&2
    exit 1
fi

SEARCHKERNEL_RUNTIME_ADAPTER_LIB="$TMP/sdk/lib/libsearchkernel_vespa.so.2" \
SEARCHKERNEL_RUNTIME_BASE_SDK="$TMP/sdk" \
LD_LIBRARY_PATH="$TMP/broken/lib" \
    "$LAUNCHER" native "$TMP/native-probe"

NATIVE_ADAPTER_LIB="$TMP/sdk/lib" \
SEARCHKERNEL_RUNTIME_BASE_SDK="$TMP/sdk" \
LD_LIBRARY_PATH="$TMP/broken/lib" \
    "$LAUNCHER" native "$TMP/native-probe"

SEARCHKERNEL_VERIFIER_SDK="$TMP/sdk" \
SEARCHKERNEL_VERIFIER_BASE_SDK="$TMP/sdk" \
LD_LIBRARY_PATH="$TMP/broken/lib" \
    "$LAUNCHER" native "$TMP/native-probe"

echo "canonical runtime launcher regression PASS"
