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

# Proton requests these configs even when the corresponding feature is unused.
# Keep explicit empty placeholders in the packaged config bundle so Vespa does
# not flood verifier logs with benign filesourcefactory warnings.
for config_name in \
    imported-fields.cfg \
    onnx-models.cfg \
    ranking-constants.cfg \
    ranking-expressions.cfg; do
    if [ ! -f "$CONFIG/$config_name" ]; then
        echo "M0 missing required config placeholder: $CONFIG/$config_name" >&2
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

LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TARGET/probe" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m0/expected/01-native-open-close.json" \
    "$TARGET/actual.json"

echo "M0 PASS"
