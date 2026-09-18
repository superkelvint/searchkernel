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

# The output directory contains only FlatBuffers-generated files.
rm -rf "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel"

$flatc_bin \
    --rust \
    --rust-module-root-file \
    -o "$repo_root/core/crates/searchkernel-protocol/src/generated" \
    "$repo_root/api/searchkernel.fbs"

generated_envelopes=$(mktemp -d)
trap 'rm -rf "$generated_envelopes"' EXIT
$flatc_bin --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/request" \
    "$repo_root/api/searchkernel_request.fbs"
$flatc_bin --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/response" \
    "$repo_root/api/searchkernel_response.fbs"
mkdir -p "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/protocol/v2"
cp "$generated_envelopes/request/search_kernel/protocol/v2/"*_generated.rs \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/protocol/v2/"
cp "$generated_envelopes/response/search_kernel/protocol/v2/"*_generated.rs \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/protocol/v2/"

# `create` is a valid portable field name, but flatc's Rust backend emits it as
# an inherent accessor that collides with every table's `create` constructor.
# Keep the wire name intact and deterministically escape only those accessors.
for generated in \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/v2/tensor_modify_update_generated.rs" \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/v2/update_request_generated.rs"; do
    perl -0pi -e 's/pub fn create\(&self\)/pub fn create_\(&self\)/g; s/self\.create\(\)/self.create_()/g' "$generated"
done

# Keep checked-in generated code consistent with the repository's Rust style.
rustfmt "$repo_root/core/crates/searchkernel-protocol/src/lib.rs" \
    "$repo_root/core/crates/searchkernel-protocol/src/protocol.rs" \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/mod.rs" \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/v2/"*_generated.rs \
    "$repo_root/core/crates/searchkernel-protocol/src/generated/search_kernel/protocol/v2/"*_generated.rs
