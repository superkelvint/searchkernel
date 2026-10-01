#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
    echo "usage: $0 <archive> <build-env> <target> <vespa-commit> <isolated-work-dir>" >&2
    exit 2
fi

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
ARCHIVE=$(cd -- "$(dirname -- "$1")" && pwd -P)/$(basename -- "$1")
BUILD_ENV=$2
TARGET=$3
VESPA_COMMIT=$4
WORK=$5
PYTHON_BIN=$(bash "$ROOT/tools/resolve-python.sh")
SOURCE_COMMIT=$(git -C "$ROOT" rev-parse HEAD)

[[ -s "$ARCHIVE" ]] || { echo "native-base archive is missing: $ARCHIVE" >&2; exit 1; }
[[ ! -e "$WORK" ]] || { echo "isolated native-base verification directory already exists: $WORK" >&2; exit 1; }
mkdir -p "$WORK/cache" "$WORK/adapter"
CONSUMER=$WORK/clean-consumer
git clone -q --no-hardlinks --no-recurse-submodules "$ROOT" "$CONSUMER"
git -C "$CONSUMER" checkout -q --detach "$SOURCE_COMMIT"
rm -rf "$CONSUMER/upstream/vespa" "$CONSUMER/vespa-deps" \
    "$CONSUMER/core/native/sdk" "$CONSUMER/target"
PYTHON_BIN=$(bash "$CONSUMER/tools/resolve-python.sh")

VESPA_ROOT=$(env -u LD_LIBRARY_PATH -u LD_PRELOAD -u SK_NATIVE_SDK \
    bash "$CONSUMER/tools/install-native-base-artifact.sh" \
    "file://$ARCHIVE" "$WORK/cache" "$BUILD_ENV" "$TARGET" "$VESPA_COMMIT")
BASE=$WORK/cache/objects/$VESPA_COMMIT
DEPS=$WORK/cache/toolchains/$VESPA_COMMIT/vespa-deps

[[ "$VESPA_ROOT" == "$WORK/cache/toolchains/$VESPA_COMMIT/upstream/vespa" ]] || {
    echo "archive installer returned unexpected toolchain root: $VESPA_ROOT" >&2
    exit 1
}
[[ ! -e "$CONSUMER/upstream/vespa" && ! -e "$CONSUMER/vespa-deps" && ! -e "$CONSUMER/core/native/sdk" ]] || {
    echo "consumer checkout contains producer native state" >&2
    exit 1
}

env -u LD_LIBRARY_PATH -u LD_PRELOAD -u SK_NATIVE_SDK \
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" SK_NATIVE_TARGET="$TARGET" \
    bash "$CONSUMER/tools/build-native-base-adapter.sh" \
    "$BASE" "$VESPA_ROOT" "$DEPS" "$WORK/adapter"

EXPECTED_ABI=$(sed -nE 's/^#define SK_NATIVE_ABI_VERSION ([0-9]+)U$/\1/p' \
    "$CONSUMER/core/native/vespa/searchkernel_vespa.h")
[[ "$EXPECTED_ABI" =~ ^[0-9]+$ ]] || { echo "cannot determine Search Kernel native ABI version" >&2; exit 1; }
env -u LD_LIBRARY_PATH -u LD_PRELOAD -u SK_NATIVE_SDK \
    "$PYTHON_BIN" "$CONSUMER/tools/smoke-native-base-adapter.py" \
    "$WORK/adapter/sdk/lib/libsearchkernel_vespa.so.2" "$EXPECTED_ABI"

echo "native-base archive consumer verification: PASS ($ARCHIVE)"
