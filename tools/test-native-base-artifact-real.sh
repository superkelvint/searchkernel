#!/usr/bin/env bash
set -euo pipefail
trap 'echo "real native base artifact regression failed at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BUILD_ENV=${SEARCHKERNEL_BUILD_ENV:-host}
TARGET=${SK_NATIVE_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
[[ -n "$TARGET" ]] || { echo "could not determine native target" >&2; exit 1; }

fixture=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-base-real.XXXXXX")
cleanup() {
    chmod -R u+w "$fixture" 2>/dev/null || true
    rm -rf "$fixture"
}
trap cleanup EXIT

# The publisher side may use the configured local Vespa checkout. The consumer
# side below is a genuinely separate clone with its own empty machine cache.
SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
SK_NATIVE_TARGET="$TARGET" \
SK_NATIVE_DISABLE_PUBLISHED_BASE=1 \
    bash "$ROOT/tools/resolve-native-sdk.sh" ensure >/dev/null

mapfile -t packaged < <(
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    bash "$ROOT/tools/package-native-base-artifact.sh" "$fixture/published"
)
[[ "${#packaged[@]}" -ge 2 ]]
archive=${packaged[0]}
checksum=${packaged[1]}
[[ -s "$archive" && -s "$checksum" ]]

head=$(git -C "$ROOT" rev-parse HEAD)
clone="$fixture/fresh-clone"
git clone -q --no-hardlinks "$ROOT" "$clone"
git -C "$clone" checkout -q "$head"

# A fresh consumer must not inherit the publisher's untracked/configured native
# state. The published artifact is the only allowed native prerequisite source.
rm -rf "$clone/upstream/vespa" "$clone/vespa-deps" "$clone/core/native/sdk" "$clone/target"
cache_home="$fixture/cache-home"
artifact_url="file://$archive"

(
    cd "$clone"
    unset SK_NATIVE_SDK VESPA_ROOT VESPA_DEPS VESPA_DEPS_LIB_DIR VESPA_EXTRA_LIB_DIR VESPA_AWS_LIB_DIR
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    XDG_CACHE_HOME="$cache_home" \
    SK_NATIVE_BASE_ARTIFACT_URL="$artifact_url" \
    SK_NATIVE_CACHE_BUILDER=/bin/false \
        bash tools/resolve-native-sdk.sh ensure >/dev/null
)

base=$(
    cd "$clone"
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    XDG_CACHE_HOME="$cache_home" \
        bash tools/resolve-native-sdk.sh path
)
vespa_root=$(
    cd "$clone"
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    XDG_CACHE_HOME="$cache_home" \
        bash tools/resolve-native-sdk.sh vespa-root
)
[[ "$base" == "$cache_home/searchkernel/native-sdk-cache/$BUILD_ENV/$TARGET/objects/"* ]]
[[ "$vespa_root" == "$cache_home/searchkernel/native-sdk-cache/$BUILD_ENV/$TARGET/toolchains/"*"/upstream/vespa" ]]
[[ -f "$vespa_root/.searchkernel-vespa-artifact.json" ]]
[[ ! -e "$clone/upstream/vespa" ]]
[[ ! -e "$clone/vespa-deps" ]]

# This is the decisive consumer proof: compile the Search Kernel adapter from
# the fresh clone using only the installed immutable base/toolchain. The local
# Vespa cache builder remains disabled, so a hidden rebuild cannot false-green.
(
    cd "$clone"
    unset SK_NATIVE_SDK VESPA_ROOT VESPA_DEPS VESPA_DEPS_LIB_DIR VESPA_EXTRA_LIB_DIR VESPA_AWS_LIB_DIR
    SEARCHKERNEL_BUILD_ENV="$BUILD_ENV" \
    SK_NATIVE_TARGET="$TARGET" \
    XDG_CACHE_HOME="$cache_home" \
    SK_NATIVE_BASE_ARTIFACT_URL="$artifact_url" \
    SK_NATIVE_CACHE_BUILDER=/bin/false \
    CARGO_TARGET_DIR="$fixture/cargo-target" \
        cargo check -p searchkernel-vespa-bindings --features native --lib
)

echo 'real native base artifact regression PASS'