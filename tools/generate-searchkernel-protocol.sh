#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$repo_root/tools/resolve-flatc.sh")
output="${SEARCHKERNEL_PROTOCOL_GENERATED_DIR:-$repo_root/core/crates/searchkernel-protocol/src/generated}"
checked_in="$repo_root/core/crates/searchkernel-protocol/src/generated"

mkdir -p "$output"

# The output directory contains only FlatBuffers-generated files.
rm -rf "$output/search_kernel"

"$flatc_bin" \
    --rust \
    --rust-module-root-file \
    -o "$output" \
    "$repo_root/api/searchkernel.fbs"

generated_envelopes=$(mktemp -d)
trap 'rm -rf "$generated_envelopes"' EXIT
"$flatc_bin" --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/request" \
    "$repo_root/api/searchkernel_request.fbs"
"$flatc_bin" --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/response" \
    "$repo_root/api/searchkernel_response.fbs"
mkdir -p "$output/search_kernel/protocol/v2"
cp "$generated_envelopes/request/search_kernel/protocol/v2/"*_generated.rs \
    "$output/search_kernel/protocol/v2/"
cp "$generated_envelopes/response/search_kernel/protocol/v2/"*_generated.rs \
    "$output/search_kernel/protocol/v2/"

# `create` is a valid portable field name, but flatc's Rust backend emits it as
# an inherent accessor that collides with every table's `create` constructor.
# Keep the wire name intact and deterministically escape only those accessors.
for generated in \
    "$output/search_kernel/v2/tensor_modify_update_generated.rs" \
    "$output/search_kernel/v2/update_request_generated.rs"; do
    perl -0pi -e 's/pub fn create\(&self\)/pub fn create_\(&self\)/g; s/self\.create\(\)/self.create_()/g' "$generated"
done

# Keep generated code deterministic regardless of whether the destination is the
# checked-in tree or a temporary verification directory.
rustfmt "$output/mod.rs" \
    "$output/search_kernel/v2/"*_generated.rs \
    "$output/search_kernel/protocol/v2/"*_generated.rs

if [[ "$output" == "$checked_in" ]]; then
    rustfmt "$repo_root/core/crates/searchkernel-protocol/src/lib.rs" \
        "$repo_root/core/crates/searchkernel-protocol/src/protocol.rs"
fi
