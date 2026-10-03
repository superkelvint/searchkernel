#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$ROOT/common/verify-common.sh"

prepare_native_probe_sdk
require_pinned_vespa

TARGET=$REPO_ROOT/target/vespa-verifier/m0
mkdir -p "$TARGET"
rm -rf "$TARGET/data"
mkdir -p "$TARGET/data"

HEADER="$SDK/include/searchkernel_vespa.h"
LIB="$SDK/lib/libsearchkernel_vespa.so"
CONFIG="$SDK/share/embedded_crud_cfg"
MANIFEST="$SDK/manifest.json"

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

require_sdk_manifest "$MANIFEST"

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

PROBE_STDERR="$TARGET/probe.stderr"
if ! LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TARGET/probe" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json" 2>"$PROBE_STDERR"; then
    cat "$PROBE_STDERR" >&2
    exit 1
fi
if grep -Eq "Filename '(imported-fields|onnx-models|ranking-constants|ranking-expressions)\\.cfg' was expected in the spec, but does not exist\\." "$PROBE_STDERR"; then
    cat "$PROBE_STDERR" >&2
    echo "M0 leaked Vespa missing optional-config warnings" >&2
    exit 1
fi
cat "$PROBE_STDERR" >&2

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m0/expected/01-native-open-close.json" \
    "$TARGET/actual.json"

echo "M0 PASS"
