#!/usr/bin/env bash
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
WORK=${P0_WORK_DIR:-"$REPO/target/python-p0-verifier"}
SDK=${SK_NATIVE_SDK:-"$REPO/core/native/sdk/linux-x86_64"}

mkdir -p "$WORK"

command -v python3 >/dev/null || { echo "python3 is required" >&2; exit 1; }
command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }

(
  cd "$REPO"
  cargo build -q -p searchkernel-bindgen
)
BINDGEN_TARGET=$(cd "$REPO" && cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
BINDGEN="$BINDGEN_TARGET/debug/searchkernel-bindgen"
[[ -x "$BINDGEN" ]] || { echo "searchkernel-bindgen executable not found: $BINDGEN" >&2; exit 1; }
BFBS_ROOT="$REPO/core/crates/searchkernel-json/bfbs"
GENERATED_OUTPUT=$(mktemp -d "$WORK/generated.XXXXXX")
"$BINDGEN" generate --bfbs-root "$BFBS_ROOT" --output-root "$GENERATED_OUTPUT"
"$BINDGEN" check --bfbs-root "$BFBS_ROOT" --repository-root "$REPO"
python3 "$REPO/tools/check-python-generated-model-construction.py" \
  "$REPO/clients/python/searchkernel"

python3 -m py_compile \
  "$REPO/clients/python/searchkernel/client.py" \
  "$REPO/clients/python/searchkernel/transport.py" \
  "$REPO/clients/python/searchkernel/generated/client.py" \
  "$HERE/acceptance.py"

if rg -n 'searchkernel-(vespa|ir-adapter|engine)|RecordingBackend|ScriptedBackend|FlatBufferBuilder|ctypes' \
    "$REPO/clients/python/searchkernel/client.py"; then
  echo "P0 handwritten façade contains a forbidden lower-level or protocol path" >&2
  exit 1
fi

(cd "$REPO" && cargo build -p searchkernel-ffi --features production)
LIB=${SEARCHKERNEL_NATIVE_LIBRARY:-"$REPO/target/debug/libsearchkernel_ffi.so"}
if [[ ! -f "$LIB" ]]; then
  echo "production FFI library not found: $LIB" >&2
  exit 1
fi
if ! nm -D --defined-only "$LIB" | rg -q ' sk_create$'; then
  echo "production FFI library does not export sk_create" >&2
  exit 1
fi
if [[ ! -f "$SDK/lib/libvespalib.so" ]]; then
  echo "Vespa runtime library not found: $SDK/lib/libvespalib.so" >&2
  exit 1
fi

export LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export LD_PRELOAD="$SDK/lib/libvespalib.so${LD_PRELOAD:+:$LD_PRELOAD}"

run_acceptance() {
  local python_path=$1
  local label=$2
  local output
  output=$(mktemp -d "$WORK/$label.XXXXXX")
  (cd "$output" && PYTHONPATH="$python_path" python3 "$HERE/acceptance.py")
  test -f "$output/tmp/products/manifest/index.json"
  test -d "$output/tmp/products/data"
}

export SEARCHKERNEL_NATIVE_LIBRARY="$LIB"
run_acceptance "$REPO/clients/python" override

WHEEL=$(mktemp -d "$WORK/wheel.XXXXXX")
SEARCHKERNEL_NATIVE_LIBRARY="$LIB" \
  python3 -m pip wheel --no-deps --no-build-isolation \
  "$REPO/clients/python" -w "$WHEEL" >/dev/null
WHEEL_NAME=$(basename "$WHEEL"/*.whl)
if [[ "$WHEEL_NAME" != *-linux_x86_64.whl ]]; then
  echo "native wheel is not tagged for Linux x86-64: $WHEEL_NAME" >&2
  exit 1
fi
SITE=$(mktemp -d "$WORK/site.XXXXXX")
unzip -q "$WHEEL"/*.whl -d "$SITE"
test -f "$SITE/searchkernel/native/linux-x86_64/libsearchkernel_ffi.so"
unset SEARCHKERNEL_NATIVE_LIBRARY
run_acceptance "$SITE" bundled

set +e
SEARCHKERNEL_NATIVE_LIBRARY="$WORK/missing.so" \
  PYTHONPATH="$REPO/clients/python" python3 -c \
  'from searchkernel.client import _resolve_native_library; _resolve_native_library()' \
  >"$WORK/missing.out" 2>"$WORK/missing.err"
status=$?
set -e
if [[ $status -eq 0 ]] || ! rg -q 'does not name a file' "$WORK/missing.err"; then
  echo "missing native-library override was not rejected clearly" >&2
  exit 1
fi

echo "P0 PASS"
