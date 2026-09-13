#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
expected_version='25.12.19'
if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=flatc
else
    flatc_bin=$(find "$repo_root/target/debug/build" -path '*/flatbuffers-build-*/out/bin/flatc' -type f -print -quit)
fi

actual_version=$($flatc_bin --version)
expected_output="flatc version ${expected_version}"
if [[ "$actual_version" != "$expected_output" ]]; then
    printf 'error: expected %s, got %s\n' "$expected_output" "$actual_version" >&2
    exit 1
fi

$flatc_bin \
    --rust \
    --rust-module-root-file \
    -o "$repo_root/crates/searchkernel-api/src/generated" \
    "$repo_root/api/searchkernel.fbs"

generated_transport=$(mktemp -d)
trap 'rm -rf "$generated_transport"' EXIT
$flatc_bin \
    --rust \
    --rust-module-root-file \
    --gen-all \
    -o "$generated_transport" \
    "$repo_root/api/searchkernel_ffi_request.fbs" \
    "$repo_root/api/searchkernel_ffi_response.fbs"

for destination in \
    "$repo_root/crates/searchkernel-ffi/generated/search_kernel/ffi/v1" \
    "$repo_root/crates/searchkernel-json/src/generated/search_kernel/ffi/v1"; do
    cp "$generated_transport/search_kernel/ffi/v1"/*_generated.rs "$destination/"
done

FLATC_PATH="$flatc_bin" cargo run -q -p searchkernel-bindgen -- generate

# Keep checked-in generated code consistent with the repository's Rust style.
rustfmt "$repo_root/crates/searchkernel-api/src/lib.rs" \
    "$repo_root/crates/searchkernel-api/src/generated/mod.rs" \
    "$repo_root/crates/searchkernel-api/src/generated/search_kernel/v1/"*_generated.rs \
    "$repo_root/crates/searchkernel-ffi/generated/search_kernel/ffi/v1/"*_generated.rs \
    "$repo_root/crates/searchkernel-json/src/generated/search_kernel/ffi/v1/"*_generated.rs \
    "$repo_root/crates/searchkernel-json/src/generated_codec.rs"
