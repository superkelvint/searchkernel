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
grep -Fq 'cp -L -f -t "$SDK/lib"' "$root/core/native/vespa/build-sdk.sh"
grep -q 'refusing to publish native SDK containing shared-library symlinks' "$root/core/native/vespa/build-sdk.sh"
grep -q 'rpath-link' "$root/core/crates/searchkernel-vespa-bindings/build.rs"
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

# Independent consumers may compile concurrently, but both must treat the
# installed FlatBuffers toolchain as read-only. Neither gets a private native
# FlatBuffers build.
build_protocol "$test_root/concurrent-a" >"$test_root/a.log" 2>&1 &
pid_a=$!
build_protocol "$test_root/concurrent-b" >"$test_root/b.log" 2>&1 &
pid_b=$!
wait "$pid_a"
wait "$pid_b"
! find "$test_root/concurrent-a" "$test_root/concurrent-b" -type f \
    -path '*/flatbuffers-build-*/out/bin/flatc' -print -quit | grep -q .
! find "$test_root/concurrent-a" "$test_root/concurrent-b" -type f \
    -path '*/flatbuffers-build-*/out/lib/libflatbuffers.a' -print -quit | grep -q .
echo 'native build hardening: concurrent Cargo builds reuse installed FlatBuffers'

bash "$root/tools/test-native-sdk-cache.sh"

echo 'native build hardening regression PASS'
