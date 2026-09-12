#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../.." && pwd)
SDK=${SK_NATIVE_SDK:-$REPO_ROOT/native/sdk/linux-x86_64}
TARGET=$REPO_ROOT/target/vespa-verifier/m0

mkdir -p "$TARGET"
rm -rf "$TARGET/data"
mkdir -p "$TARGET/data"

HEADER=$SDK/include/searchkernel_vespa.h
LIB=$SDK/lib/libsearchkernel_vespa.so
CONFIG=$SDK/share/embedded_crud_cfg
MANIFEST=$SDK/manifest.json

if grep -qE "[#]include[[:space:]]*[<\"]vespa/" "$HEADER"; then
    echo "M0 public native header leaks Vespa headers" >&2
    exit 1
fi

for path in "$HEADER" "$LIB" "$CONFIG" "$MANIFEST"; do
    if [ ! -e "$path" ]; then
        echo "M0 missing required SDK artifact: $path" >&2
        exit 1
    fi
done

python3 - "$MANIFEST" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
assert m.get("native_abi_version") == 1, m
assert m.get("vespa_commit") == "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e", m
PY

if ldd "$LIB" | grep -q 'not found'; then
    echo "M0 native library has unresolved runtime dependencies:" >&2
    ldd "$LIB" >&2
    exit 1
fi

cc -std=c11 -Wall -Wextra -Werror \
    -I"$SDK/include" \
    "$ROOT/m0/native-probe/probe.c" \
    -L"$SDK/lib" -lsearchkernel_vespa \
    -Wl,-rpath,"$SDK/lib" \
    -o "$TARGET/probe"

LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TARGET/probe" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m0/expected/01-native-open-close.json" \
    "$TARGET/actual.json"

echo "M0 PASS"
