#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
test_target=$(mktemp -d "${TMPDIR:-/tmp}/searchkernel-native-build.XXXXXX")
trap 'rm -rf "$test_target"' EXIT

grep -q 'target-dir = "target/host"' "$root/.cargo/config.toml"
grep -q 'ENV CARGO_TARGET_DIR=target/devcontainer' "$root/.yolobox.Dockerfile"
grep -q 'CARGO_TARGET_DIR: target/ci' "$root/.github/workflows/abi-surface.yml"
echo 'native build hardening: host, devcontainer, and CI target directories are distinct'

native_target="$test_target/cargo"
build_filter() {
    CARGO_TARGET_DIR="$native_target" \
    SEARCHKERNEL_BUILD_ENV=native-build-regression \
        cargo build --manifest-path "$root/Cargo.toml" -p searchkernel-filter
}

build_protocol() {
    local flags=$1
    local rustflags=$2
    if [[ "$rustflags" == unset ]]; then
        env -u CMAKE_GENERATOR -u RUSTFLAGS \
            CARGO_TARGET_DIR="$native_target" \
            SEARCHKERNEL_BUILD_ENV=native-build-regression \
            CXXFLAGS="$flags" \
                cargo check --manifest-path "$root/Cargo.toml" -p searchkernel-protocol \
                    --no-default-features
    else
        env -u CMAKE_GENERATOR \
        RUSTFLAGS="$rustflags" \
        CARGO_TARGET_DIR="$native_target" \
        SEARCHKERNEL_BUILD_ENV=native-build-regression \
        CXXFLAGS="$flags" \
            cargo check --manifest-path "$root/Cargo.toml" -p searchkernel-protocol \
                --no-default-features
    fi
}

find_native_output() {
    find "$native_target/debug/build" -path '*/flatbuffers-build-*/out' -type d -print -quit
}

build_filter
base_flags=${CXXFLAGS-}
build_protocol "$base_flags" unset
native_output=$(find_native_output)
[[ -n "$native_output" ]] || { echo 'native FlatBuffers output was not found' >&2; exit 1; }
fingerprint="$native_output/native-build-fingerprint-v1.txt"
archive="$native_output/lib/libflatbuffers.a"
probe="$native_output/probe/flatbuffers_link_probe"
[[ -s "$fingerprint" && -s "$archive" && -x "$probe" ]]

first_fingerprint=$(sha256sum "$fingerprint")
first_archive=$(sha256sum "$archive")
first_archive_inode=$(stat -c '%i' "$archive")
unrelated=$(find "$native_target/debug/deps" -maxdepth 1 -name 'libsearchkernel_filter-*.rlib' -type f -print -quit)
[[ -n "$unrelated" && -f "$unrelated" ]] || { echo 'unrelated Rust artifact was not built' >&2; exit 1; }

# An unchanged environment must retain both the native output and unrelated Rust output.
build_protocol "$base_flags" unset
[[ "$first_fingerprint" == "$(sha256sum "$fingerprint")" ]]
[[ "$first_archive" == "$(sha256sum "$archive")" ]]
[[ -f "$unrelated" ]]
echo 'native build hardening: unchanged environment reuses FlatBuffers output'

# A relevant native flag changes the fingerprint and replaces only the native output.
changed_flags="$base_flags -DSEARCHKERNEL_NATIVE_BUILD_REGRESSION=1"
build_protocol "$changed_flags" unset
second_archive_inode=$(stat -c '%i' "$archive")
[[ "$first_fingerprint" != "$(sha256sum "$fingerprint")" ]]
[[ "$first_archive_inode" != "$second_archive_inode" ]]
[[ -f "$unrelated" ]]
echo 'native build hardening: changed CXXFLAGS rebuilt the native component only'

# Corrupt output, then make Cargo rerun the native builder without changing its
# fingerprint. The real parser/link probe must fail once and recover automatically.
truncate -s 1 "$archive"
build_protocol "$changed_flags" '-C debuginfo=0'
[[ -s "$archive" && -x "$probe" ]]
echo 'native build hardening: failed link probe triggered one targeted recovery'

echo 'native build hardening regression PASS'
