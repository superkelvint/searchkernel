#!/usr/bin/env bash
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)

C5_VERIFY=${C5_VERIFY:-"$REPO/reference/ffi-conformance/verify.sh"}
C6_VERIFY=${C6_VERIFY:-"$REPO/reference/json-conformance/verify.sh"}
PY_ADAPTER=${PY_ADAPTER:-"$REPO/clients/python/tests/c7_adapter.py"}
PY_TRANSPORT_SRC=${PY_TRANSPORT_SRC:-"$REPO/clients/python"}
REAL_C5_LIB=${SEARCHKERNEL_C5_TEST_LIBRARY:-}

WORK=${C7_WORK_DIR:-"$REPO/target/c7-verifier"}
mkdir -p "$WORK"

say() { printf '\n[C7] %s\n' "$*"; }
fail() { printf '\n[C7] FAIL: %s\n' "$*" >&2; exit 1; }

command -v python3 >/dev/null || fail "python3 not found"
command -v cc >/dev/null || fail "C compiler not found (needed only for verifier shim)"

say "syntax-checking frozen verifier"
python3 -m py_compile \
  "$HERE/python/transport_probe.py" \
  "$HERE/python/C7_ADAPTER_CONTRACT.py" \
  "$HERE/tools/validate_trace.py" \
  "$HERE/tools/check_thinness.py"
bash -n "$HERE/verify-c7.sh" "$HERE/shim/build.sh"

[[ -f "$PY_ADAPTER" ]] || fail "missing Python verifier adapter: $PY_ADAPTER"
[[ -d "$PY_TRANSPORT_SRC" ]] || fail "missing Python client source tree: $PY_TRANSPORT_SRC"

say "checking handwritten Python NativeTransport thinness"
python3 "$HERE/tools/check_thinness.py" "$PY_TRANSPORT_SRC"

say "building verifier-only C5 plumbing shim"
SHIM="$WORK/libsearchkernel_c7_shim.so"
"$HERE/shim/build.sh" "$SHIM" >/dev/null

PY_EXPECTED_ABI=$(python3 - <<PY
import importlib.util
p = r"$PY_ADAPTER"
s = importlib.util.spec_from_file_location("c7a", p)
if s is None or s.loader is None:
    raise SystemExit("cannot import C7 adapter")
m = importlib.util.module_from_spec(s)
s.loader.exec_module(m)
print(int(m.expected_abi_version()))
PY
)
[[ "$PY_EXPECTED_ABI" =~ ^[0-9]+$ ]] || fail "Python adapter returned invalid ABI version"

say "Python plumbing / lifecycle / ownership verifier"
PY_TRACE="$WORK/python.trace"
rm -f "$PY_TRACE"
export C7_SHIM_ABI_VERSION="$PY_EXPECTED_ABI"
export C7_SHIM_TRACE="$PY_TRACE"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" \
  --library "$SHIM" \
  --mode plumbing \
  --scenarios "$HERE/scenarios/transport.json"
python3 "$HERE/tools/validate_trace.py" "$PY_TRACE" \
  --expect-opens 6 \
  --expect-closes 6 \
  --expect-calls 5010 \
  --expect-allocs 5008 \
  --expect-frees 5008 \
  --require-abi-before-native-use

say "Python ABI mismatch verifier"
ABI_TRACE="$WORK/python-abi-mismatch.trace"
rm -f "$ABI_TRACE"
export C7_SHIM_ABI_VERSION=4294967295
export C7_SHIM_TRACE="$ABI_TRACE"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$SHIM" --mode abi-mismatch
python3 "$HERE/tools/validate_trace.py" "$ABI_TRACE" \
  --expect-opens 0 --expect-closes 0 --expect-calls 0 \
  --expect-allocs 0 --expect-frees 0
export C7_SHIM_ABI_VERSION="$PY_EXPECTED_ABI"

say "Python missing-library verifier"
MISSING="$WORK/definitely-does-not-exist/libsearchkernel_missing.so"
rm -rf "$(dirname "$MISSING")"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$MISSING" --mode missing-library

say "verifying prerequisite C5 and C6 contracts"
[[ -x "$C5_VERIFY" ]] || fail "C5 verifier not found/executable: $C5_VERIFY"
[[ -x "$C6_VERIFY" ]] || fail "C6 verifier not found/executable: $C6_VERIFY"
"$C5_VERIFY"
"$C6_VERIFY"

if [[ -z "$REAL_C5_LIB" ]]; then
  say "building C5 C3 composition test library"
  (cd "$REPO" && cargo build -p searchkernel-ffi --features c7-test-support)
  case "$(uname -s)" in
    Linux) REAL_C5_LIB="$REPO/target/debug/libsearchkernel_ffi.so" ;;
    Darwin) REAL_C5_LIB="$REPO/target/debug/libsearchkernel_ffi.dylib" ;;
    *) fail "unsupported verifier host: $(uname -s)" ;;
  esac
fi
[[ -f "$REAL_C5_LIB" ]] || fail "real C5 test library does not exist: $REAL_C5_LIB"

say "generated Python client -> NativeTransport -> real C5 smoke"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$REAL_C5_LIB" --mode real-c5-smoke

say "checking Python C7 source remains independent of Vespa"
if grep -RInE 'searchkernel[-_]vespa|vespa[_-]engine' "$PY_TRANSPORT_SRC" \
     --include='*.py' --include='*.rs' 2>/dev/null; then
  fail "C7 Python client/transport source depends on Vespa"
fi

say "PASS"
