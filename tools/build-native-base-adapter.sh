#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
    echo "usage: $0 <base-sdk> <vespa-toolchain-root> <dependency-root> <adapter-output-root>" >&2
    exit 2
fi

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
BASE=$(cd -- "$1" && pwd -P)
VESPA=$(cd -- "$2" && pwd -P)
DEPS=$(cd -- "$3" && pwd -P)
OUTPUT=$4
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}

[[ -d "$BASE/lib" && -d "$DEPS/include" && -f "$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir/link.txt" ]] || {
    echo "adapter inputs are incomplete: base=$BASE vespa=$VESPA deps=$DEPS" >&2
    exit 1
}

mkdir -p "$OUTPUT"
env -u LD_LIBRARY_PATH -u LD_PRELOAD -u SK_NATIVE_SDK \
    -u SK_NATIVE_BASE_ONLY -u SK_NATIVE_EXTRA_CXX_FLAGS \
    -u SK_NATIVE_EXTRA_LINK_FLAGS -u SK_NATIVE_ENABLE_FAILPOINTS \
    -u SK_NATIVE_COVERAGE -u SK_NATIVE_SANITIZER \
    -u SK_NATIVE_SKIP_ABI_AUTHORITIES -u VESPA_LIBRARY_ROOT \
    -u VESPA_DEPS_LIB_DIR -u VESPA_DEPS_INCLUDE_DIR -u VESPA_AWS_LIB_DIR \
    -u CXX -u CC -u CXXFLAGS -u CPPFLAGS -u LDFLAGS \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    SK_NATIVE_ADAPTER_ONLY=1 \
    SK_NATIVE_BASE_SDK="$BASE" \
    SK_NATIVE_SDK="$OUTPUT/sdk" \
    SK_NATIVE_BUILD_DIR="$OUTPUT/build" \
    SK_NATIVE_REGENERATE_BINDINGS=0 \
    SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
    VESPA_ROOT="$VESPA" \
    VESPA_DEPS="$DEPS" \
    VESPA_DEPS_INCLUDE_DIR="$DEPS/include" \
    VESPA_DEPS_LIB_DIR="$DEPS/lib64" \
    VESPA_EXTRA_LIB_DIR="$BASE/lib" \
    bash "$ROOT/core/native/vespa/build-sdk.sh"

[[ -s "$OUTPUT/sdk/lib/libsearchkernel_vespa.so.2" ]] || {
    echo "adapter build produced no versioned shared library" >&2
    exit 1
}

"$PYTHON_BIN" "$ROOT/tools/check-native-runtime-closure.py" \
    "$BASE/lib" "$OUTPUT/sdk/lib/libsearchkernel_vespa.so.2" --require-provenance >&2
