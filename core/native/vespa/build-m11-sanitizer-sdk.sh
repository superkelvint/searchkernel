#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
LANE=${1:-}
SDK=${2:-}
BUILD=${3:-}

case "$LANE" in
    asan-ubsan)
        FLAGS="-fno-omit-frame-pointer -fsanitize=address,undefined"
        ;;
    tsan)
        FLAGS="-fno-omit-frame-pointer -fsanitize=thread"
        ;;
    *)
        echo "usage: $0 asan-ubsan|tsan SDK BUILD" >&2
        exit 2
        ;;
esac

[ -n "$SDK" ] && [ -n "$BUILD" ] || {
    echo "sanitizer SDK and build directories are required" >&2
    exit 2
}

SK_NATIVE_SDK="$SDK" \
SK_NATIVE_BUILD_DIR="$BUILD" \
SK_NATIVE_ENABLE_FAILPOINTS=1 \
SK_NATIVE_SANITIZER="$LANE" \
SK_NATIVE_REGENERATE_BINDINGS=0 \
SK_NATIVE_EXTRA_CXX_FLAGS="$FLAGS" \
SK_NATIVE_EXTRA_LINK_FLAGS="$FLAGS" \
    "$ROOT/core/native/vespa/build-sdk.sh"

echo "built M11 sanitizer SDK ($LANE) at $SDK"
