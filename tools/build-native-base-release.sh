#!/usr/bin/env bash
set -euo pipefail
trap 'echo "native-base artifact build failed at line $LINENO: $BASH_COMMAND" >&2' ERR

if [[ $# -lt 2 || $# -gt 3 ]]; then
    echo "usage: $0 <host|ci> <output-dir> [generation]" >&2
    exit 2
fi

ROOT=$(git rev-parse --show-toplevel)
ROOT=$(cd -- "$ROOT" && pwd -P)
BUILD_ENV=$1
OUTPUT=$(mkdir -p "$2" && cd "$2" && pwd -P)
ARTIFACT_GENERATION=${3:-${SK_NATIVE_BASE_GENERATION:-1}}
case "$BUILD_ENV" in
    host|ci) ;;
    *) echo "unsupported native-base build environment: $BUILD_ENV" >&2; exit 2 ;;
esac
[[ "$ARTIFACT_GENERATION" =~ ^[1-9][0-9]*$ ]] || {
    echo "invalid native-base generation: $ARTIFACT_GENERATION" >&2
    exit 2
}

SOURCE_COMMIT=$(git -C "$ROOT" rev-parse HEAD)
[[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=normal)" ]] || {
    echo "native-base artifact builds require a clean Search Kernel source checkout" >&2
    exit 1
}
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
RUSTC_BIN=$(bash "$ROOT/tools/resolve-rust.sh" rustc)
TARGET=${SK_NATIVE_TARGET:-$("$RUSTC_BIN" -vV | sed -n 's/^host: //p')}
PINNED_VESPA_COMMIT=$("$PYTHON_BIN" - "$ROOT/core/native/vespa/manifest.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    print(json.load(stream)["vespa_commit"])
PY
)
VESPA_INPUT=${VESPA_ROOT:-}
DEPS_INPUT=${VESPA_DEPS:-}
[[ -n "$VESPA_INPUT" && -d "$VESPA_INPUT" ]] || {
    echo "set VESPA_ROOT to the configured pinned Vespa producer checkout" >&2
    exit 1
}
[[ -n "$DEPS_INPUT" && -d "$DEPS_INPUT/include" ]] || {
    echo "set VESPA_DEPS to the dependency installation used by that Vespa build" >&2
    exit 1
}
VESPA_INPUT=$(cd -- "$VESPA_INPUT" && pwd -P)
DEPS_INPUT=$(cd -- "$DEPS_INPUT" && pwd -P)
[[ "$(git -C "$VESPA_INPUT" rev-parse HEAD)" == "$PINNED_VESPA_COMMIT" ]] || {
    echo "configured Vespa commit does not match pinned $PINNED_VESPA_COMMIT" >&2
    exit 1
}
git -C "$VESPA_INPUT" diff --quiet --ignore-submodules -- &&
git -C "$VESPA_INPUT" diff --cached --quiet --ignore-submodules -- || {
    echo "configured Vespa producer checkout is dirty" >&2
    exit 1
}

MARKER=$VESPA_INPUT/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_embedded_crud_spike_app.dir
[[ -f "$MARKER/link.txt" && -f "$MARKER/flags.make" ]] || {
    echo "configured Vespa build is missing adapter link metadata: $MARKER" >&2
    exit 1
}

TMP=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-build.XXXXXX")
cleanup() {
    local status=$?
    trap - EXIT INT TERM
    bash "$ROOT/tools/remove-readonly-tree.sh" "$TMP" || {
        (( status != 0 )) || status=1
    }
    exit "$status"
}
trap cleanup EXIT
PRODUCER_CACHE=$TMP/native-cache
mkdir -p "$PRODUCER_CACHE"

env -u SK_NATIVE_SDK -u SK_NATIVE_BASE_ARTIFACT_URL \
    -u SK_NATIVE_BASE_ARTIFACT_GENERATION -u SK_NATIVE_BASE_SDK \
    -u SK_NATIVE_ADAPTER_ONLY -u SK_NATIVE_BASE_ONLY -u SK_NATIVE_BUILD_DIR \
    -u SK_NATIVE_CACHE_BUILDER -u SK_NATIVE_EXTRA_CXX_FLAGS \
    -u SK_NATIVE_EXTRA_LINK_FLAGS -u SK_NATIVE_ENABLE_FAILPOINTS \
    -u SK_NATIVE_COVERAGE -u SK_NATIVE_SANITIZER -u SK_NATIVE_SKIP_ABI_AUTHORITIES \
    -u VESPA_LIBRARY_ROOT -u VESPA_DEPS_LIB_DIR -u VESPA_DEPS_INCLUDE_DIR \
    -u VESPA_EXTRA_LIB_DIR -u VESPA_AWS_LIB_DIR -u CXX -u CC \
    -u CXXFLAGS -u CPPFLAGS -u LDFLAGS -u LD_LIBRARY_PATH -u LD_PRELOAD \
    SEARCHKERNEL_DEV_USE_CI_IMAGE=0 \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    SK_NATIVE_DISABLE_PUBLISHED_BASE=1 \
    SK_NATIVE_CACHE_DIR="$PRODUCER_CACHE" \
    SK_NATIVE_BASE_GENERATION="$ARTIFACT_GENERATION" \
    VESPA_ROOT="$VESPA_INPUT" VESPA_DEPS="$DEPS_INPUT" \
    CARGO_TARGET_DIR="$TMP/cargo-target" \
    bash "$ROOT/dev" native-sdk ensure >/dev/null

STAGE=$TMP/stage
env -u SK_NATIVE_SDK -u SK_NATIVE_BASE_SDK -u SK_NATIVE_ADAPTER_ONLY \
    -u SK_NATIVE_BASE_ONLY -u SK_NATIVE_EXTRA_CXX_FLAGS -u SK_NATIVE_EXTRA_LINK_FLAGS \
    -u SK_NATIVE_ENABLE_FAILPOINTS -u SK_NATIVE_COVERAGE -u SK_NATIVE_SANITIZER \
    -u SK_NATIVE_SKIP_ABI_AUTHORITIES -u VESPA_LIBRARY_ROOT \
    -u VESPA_DEPS_LIB_DIR -u VESPA_DEPS_INCLUDE_DIR -u VESPA_EXTRA_LIB_DIR \
    -u VESPA_AWS_LIB_DIR -u CXX -u CC -u CXXFLAGS -u CPPFLAGS -u LDFLAGS \
    -u LD_LIBRARY_PATH -u LD_PRELOAD \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    SK_NATIVE_BASE_GENERATION="$ARTIFACT_GENERATION" \
    SK_NATIVE_BASE_SOURCE_COMMIT="$SOURCE_COMMIT" \
    SK_NATIVE_CACHE_DIR="$PRODUCER_CACHE" \
    VESPA_ROOT="$VESPA_INPUT" VESPA_DEPS="$DEPS_INPUT" \
    "$ROOT/tools/package-native-base-artifact.sh" "$OUTPUT" "$STAGE" >/dev/null
BUNDLE=$STAGE/searchkernel-vespa-base

STAGED_ADAPTER=$TMP/staged-adapter
bash "$ROOT/tools/build-native-base-adapter.sh" \
    "$BUNDLE/base" "$BUNDLE/toolchain/upstream/vespa" \
    "$BUNDLE/toolchain/vespa-deps" "$STAGED_ADAPTER"

"$PYTHON_BIN" "$ROOT/tools/prune-native-runtime-closure.py" \
    "$BUNDLE/base/lib" \
    --root-elf "$STAGED_ADAPTER/sdk/lib/libsearchkernel_vespa.so.2"
runtime_search_args=()
for runtime_dir in "$BUNDLE/toolchain/vespa-deps/lib64" "$BUNDLE/toolchain/vespa-deps/lib"; do
    if [[ -d "$runtime_dir" ]]; then
        runtime_search_args+=(--search-dir "$runtime_dir")
    fi
done
"$PYTHON_BIN" "$ROOT/tools/bundle-native-runtime-closure.py" \
    "$BUNDLE/base/lib" \
    --extra-elf "$STAGED_ADAPTER/sdk/lib/libsearchkernel_vespa.so.2" \
    --source-root "native-base=$BUNDLE/base" \
    --source-root "vespa-toolchain=$BUNDLE/toolchain/upstream/vespa" \
    --source-root "vespa-deps=$BUNDLE/toolchain/vespa-deps" \
    "${runtime_search_args[@]}"
"$PYTHON_BIN" "$ROOT/tools/write-native-base-manifest.py" \
    "$BUNDLE/manifest.json" \
    "$BUNDLE/toolchain/upstream/vespa/.searchkernel-vespa-artifact.json" \
    "$BUNDLE/base/lib/.searchkernel-runtime-provenance.json" \
    "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" \
    "$ARTIFACT_GENERATION" "$SOURCE_COMMIT"
"$PYTHON_BIN" "$ROOT/tools/check-native-runtime-closure.py" \
    "$BUNDLE/base/lib" "$STAGED_ADAPTER/sdk/lib/libsearchkernel_vespa.so.2" \
    --require-provenance
EXPECTED_ABI=$(sed -nE 's/^#define SK_NATIVE_ABI_VERSION ([0-9]+)U$/\1/p' \
    "$ROOT/core/native/vespa/searchkernel_vespa.h")
env -u LD_LIBRARY_PATH -u LD_PRELOAD \
    "$PYTHON_BIN" "$ROOT/tools/smoke-native-base-adapter.py" \
    "$STAGED_ADAPTER/sdk/lib/libsearchkernel_vespa.so.2" "$EXPECTED_ABI"

ASSET="searchkernel-vespa-base-v${ARTIFACT_GENERATION}-${BUILD_ENV}-${TARGET}-${PINNED_VESPA_COMMIT}.tar.gz"
ARCHIVE=$OUTPUT/$ASSET
CHECKSUM=$ARCHIVE.sha256
[[ ! -e "$ARCHIVE" && ! -e "$CHECKSUM" ]] || {
    echo "refusing to overwrite an existing native-base artifact: $ARCHIVE" >&2
    exit 1
}
(
    cd "$STAGE"
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 \
        --numeric-owner -cf - searchkernel-vespa-base | gzip -n >"$ARCHIVE"
)
sha256sum "$ARCHIVE" >"$CHECKSUM"

VERIFY=$TMP/archive-consumer
env -u LD_LIBRARY_PATH -u LD_PRELOAD -u SK_NATIVE_SDK \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" SK_NATIVE_TARGET="$TARGET" \
    bash "$ROOT/tools/verify-native-base-archive.sh" \
    "$ARCHIVE" "$BUILD_ENV" "$TARGET" "$PINNED_VESPA_COMMIT" "$VERIFY"

ACTUAL=$(sha256sum "$ARCHIVE" | awk '{print $1}')
printf '%s\n%s\n%s\n' "$ARCHIVE" "$CHECKSUM" "$ACTUAL"
