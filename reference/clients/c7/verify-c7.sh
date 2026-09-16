#!/usr/bin/env bash
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)

C5_VERIFY=${C5_VERIFY:-"$REPO/reference/ffi-conformance/verify-c5.sh"}
C6_VERIFY=${C6_VERIFY:-"$REPO/reference/clients/c6/verify-c6.sh"}
PY_ADAPTER=${PY_ADAPTER:-"$REPO/clients/python/tests/c7_adapter.py"}
GO_ADAPTER_DIR=${GO_ADAPTER_DIR:-"$REPO/clients/go/c7adapter"}
PY_TRANSPORT_SRC=${PY_TRANSPORT_SRC:-"$REPO/clients/python"}
GO_TRANSPORT_SRC=${GO_TRANSPORT_SRC:-"$REPO/clients/go"}

WORK=${C7_WORK_DIR:-"$REPO/target/c7-verifier"}
mkdir -p "$WORK"

say() { printf '\n[C7] %s\n' "$*"; }
fail() { printf '\n[C7] FAIL: %s\n' "$*" >&2; exit 1; }

say "syntax-checking frozen verifier"
python3 -m py_compile \
  "$HERE/python/transport_probe.py" \
  "$HERE/python/C7_ADAPTER_CONTRACT.py" \
  "$HERE/tools/validate_trace.py" \
  "$HERE/tools/check_thinness.py"
bash -n "$HERE/verify-c7.sh" "$HERE/shim/build.sh"

command -v cc >/dev/null || fail "C compiler not found"
command -v python3 >/dev/null || fail "python3 not found"
command -v go >/dev/null || fail "Go toolchain not found"

[[ -f "$PY_ADAPTER" ]] || fail "missing Python verifier adapter: $PY_ADAPTER"
[[ -d "$GO_ADAPTER_DIR" ]] || fail "missing Go verifier adapter: $GO_ADAPTER_DIR"

say "checking NativeTransport thinness"
python3 "$HERE/tools/check_thinness.py" "$PY_TRANSPORT_SRC" "$GO_TRANSPORT_SRC"

say "building verifier C5 plumbing shim"
SHIM="$WORK/libsearchkernel_c7_shim.so"
"$HERE/shim/build.sh" "$SHIM" >/dev/null

# The shim default ABI must match the implementation's expected ABI. The
# adapters expose the expected version without loading the shim, so this does
# not duplicate C5 ABI version policy in the verifier.
PY_EXPECTED_ABI=$(python3 - <<PY
import importlib.util
p = r"$PY_ADAPTER"
s = importlib.util.spec_from_file_location("c7a", p)
m = importlib.util.module_from_spec(s); s.loader.exec_module(m)
print(int(m.expected_abi_version()))
PY
)
[[ "$PY_EXPECTED_ABI" =~ ^[0-9]+$ ]] || fail "Python adapter returned invalid ABI version"
export C7_SHIM_ABI_VERSION="$PY_EXPECTED_ABI"

say "Python plumbing/ownership verifier"
PY_TRACE="$WORK/python.trace"
export C7_SHIM_TRACE="$PY_TRACE"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$SHIM" --mode plumbing \
  --scenarios "$HERE/scenarios/transport.json"
python3 "$HERE/tools/validate_trace.py" "$PY_TRACE" --min-calls 5008

say "Python ABI mismatch verifier"
export C7_SHIM_ABI_VERSION=4294967295
export C7_SHIM_TRACE="$WORK/python-abi-mismatch.trace"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$SHIM" --mode abi-mismatch
export C7_SHIM_ABI_VERSION="$PY_EXPECTED_ABI"

say "Go plumbing/ownership verifier"
GO_TRACE="$WORK/go.trace"
export C7_SHIM_TRACE="$GO_TRACE"
(
  cd "$REPO"
  go run "$HERE/go/transport_probe.go" \
    --mode plumbing --library "$SHIM" --scenarios "$HERE/scenarios/transport.json"
)
python3 "$HERE/tools/validate_trace.py" "$GO_TRACE" --min-calls 5008

say "Go ABI mismatch verifier"
export C7_SHIM_ABI_VERSION=4294967295
export C7_SHIM_TRACE="$WORK/go-abi-mismatch.trace"
(
  cd "$REPO"
  go run "$HERE/go/transport_probe.go" --mode abi-mismatch --library "$SHIM"
)
export C7_SHIM_ABI_VERSION="$PY_EXPECTED_ABI"

say "verifying prerequisite C5 and C6 contracts"
[[ -x "$C5_VERIFY" ]] || fail "C5 verifier not found/executable: $C5_VERIFY"
[[ -x "$C6_VERIFY" ]] || fail "C6 verifier not found/executable: $C6_VERIFY"
"$C5_VERIFY"
"$C6_VERIFY"

# The repository's C5 verifier/build step must publish the actual test library
# path. This avoids guessing Cargo target filenames in C7.
REAL_C5_LIB=${SEARCHKERNEL_C5_TEST_LIBRARY:-}
[[ -n "$REAL_C5_LIB" ]] || fail "C5 verification must export SEARCHKERNEL_C5_TEST_LIBRARY for C7 real-C5 smoke"
[[ -f "$REAL_C5_LIB" ]] || fail "real C5 test library does not exist: $REAL_C5_LIB"

say "Python generated-client -> NativeTransport -> real C5 smoke"
python3 "$HERE/python/transport_probe.py" \
  --repo "$REPO" --library "$REAL_C5_LIB" --mode real-c5-smoke

say "Go generated-client -> NativeTransport -> real C5 smoke"
(
  cd "$REPO"
  go run "$HERE/go/transport_probe.go" --mode real-c5-smoke --library "$REAL_C5_LIB"
)

say "checking C-series independence from Vespa in client transports"
if grep -RInE 'searchkernel[-_]vespa|vespa[_-]engine' "$REPO/clients/python" "$REPO/clients/go" \
     --include='*.py' --include='*.go' --include='*.rs' 2>/dev/null; then
  fail "C7 language client/transport source depends on Vespa"
fi

say "PASS"
