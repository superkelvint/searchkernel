#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-build.XXXXXX")
trap 'rm -rf "$test_root"' EXIT

grep -q 'target-dir = "target/host"' "$root/.cargo/config.toml"
! grep -q 'core/crates/flatbuffers-build' "$root/Cargo.toml"
! grep -q 'flatbuffers-build' "$root/core/crates/searchkernel-protocol/Cargo.toml"
grep -q 'FLATBUFFERS_CPP_ROOT' "$root/core/crates/searchkernel-protocol/build.rs"
grep -q "Cargo does not build FlatBuffers itself" "$root/core/crates/searchkernel-protocol/build.rs"

grep -q 'ENV CARGO_TARGET_DIR=target/devcontainer' "$root/.yolobox.Dockerfile"
grep -q 'CARGO_TARGET_DIR: target/ci' "$root/.github/workflows/abi-surface.yml"
grep -q 'git -C "\$ROOT" worktree list' "$root/core/native/vespa/build-sdk.sh"
grep -q 'SDK_PACKAGE/lib/libLLVM.so.21.1' "$root/core/native/vespa/build-sdk.sh"
grep -q 'libffi.so.6 required by the bundled LLVM runtime' "$root/core/native/vespa/build-sdk.sh"
grep -q 'copy-shared-library-into-sdk.sh' "$root/core/native/vespa/build-sdk.sh"
grep -q 'refusing to publish native SDK containing shared-library symlinks' "$root/core/native/vespa/build-sdk.sh"
grep -q 'rpath-link' "$root/core/crates/searchkernel-vespa-bindings/build.rs"
grep -q 'SK_NATIVE_BASE_ONLY' "$root/core/native/vespa/build-sdk.sh"
grep -Fq 'LINK_ARGS="-L$BASE_SDK/lib $LINK_ARGS"' "$root/core/native/vespa/build-sdk.sh"
grep -Fq 's#[^ ]*/$LIB_NAME#$BASE_LIBRARY#g' "$root/core/native/vespa/build-sdk.sh"
grep -q "refusing to overwrite Vespa base" "$root/core/native/vespa/build-sdk.sh"
grep -q -- '--scope base' "$root/tools/resolve-native-sdk.sh"
bash "$root/tools/test-native-sdk-packaging.sh"
echo 'native build hardening: host, devcontainer, and CI target directories are distinct'

flatc=$(bash "$root/tools/resolve-flatc.sh")
flatbuffers_root=${FLATBUFFERS_CPP_ROOT:-$(cd -- "$(dirname -- "$flatc")/.." && pwd)}
[[ -f "$flatbuffers_root/include/flatbuffers/base.h" ]]
[[ -f "$flatbuffers_root/lib/libflatbuffers.a" ]]

build_protocol() {
    local target=$1
    FLATBUFFERS_CPP_ROOT="$flatbuffers_root" \
    CMAKE=/bin/false \
    CARGO_TARGET_DIR="$target" \
    SEARCHKERNEL_BUILD_ENV=native-build-regression \
        cargo check --manifest-path "$root/Cargo.toml" -p searchkernel-protocol --no-default-features
}

# A completely fresh Cargo target must consume the installed FlatBuffers
# prerequisite. Setting CMAKE=/bin/false makes any accidental return of the
# vendored CMake build path fail immediately.
fresh_target="$test_root/fresh"
build_protocol "$fresh_target"
! find "$fresh_target" -type f -path '*/flatbuffers-build-*/out/bin/flatc' -print -quit | grep -q .
! find "$fresh_target" -type f -path '*/flatbuffers-build-*/out/lib/libflatbuffers.a' -print -quit | grep -q .
echo 'native build hardening: fresh Cargo target does not build FlatBuffers'

# One fresh target is sufficient: the structural guards above prove the vendored
# builder is absent, while CMAKE=/bin/false and the artifact checks prove a clean
# Cargo consumer cannot recreate a private FlatBuffers native build. Do not add
# parallel empty target directories here; that defeats Cargo incremental reuse.
bash "$root/tools/test-native-sdk-cache.sh"

echo 'native build hardening regression PASS'
