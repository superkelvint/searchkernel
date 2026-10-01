#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$repo_root/tools/resolve-flatbuffers.sh" flatc)
rustfmt_bin=$(bash "$repo_root/tools/resolve-rust.sh" rustfmt)
generated_root=${SEARCHKERNEL_RUST_GENERATED_DIR:-"$repo_root/core/crates/searchkernel-protocol/src/generated"}

# The output directory contains only FlatBuffers-generated files.
mkdir -p "$generated_root"
rm -rf "$generated_root/search_kernel"

"$flatc_bin" \
    --rust \
    --rust-module-root-file \
    -o "$generated_root" \
    "$repo_root/api/wire/v1/searchkernel.fbs"

generated_envelopes=$(mktemp -d)
trap 'rm -rf "$generated_envelopes"' EXIT
"$flatc_bin" --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/request" \
    "$repo_root/api/wire/v1/searchkernel_request.fbs"
"$flatc_bin" --rust --rust-module-root-file --gen-all \
    -o "$generated_envelopes/response" \
    "$repo_root/api/wire/v1/searchkernel_response.fbs"
mkdir -p "$generated_root/search_kernel/protocol"
cp "$generated_envelopes/request/search_kernel/protocol/"*_generated.rs \
    "$generated_root/search_kernel/protocol/"
cp "$generated_envelopes/response/search_kernel/protocol/"*_generated.rs \
    "$generated_root/search_kernel/protocol/"

# `create` is a valid portable field name, but flatc's Rust backend emits it as
# an inherent accessor that collides with every table's `create` constructor.
# Keep the wire name intact and deterministically escape only those accessors.
for generated in \
    "$generated_root/search_kernel/wire/tensor_modify_update_generated.rs" \
    "$generated_root/search_kernel/wire/update_request_generated.rs"; do
    perl -0pi -e 's/pub fn create\(&self\)/pub fn create_\(&self\)/g; s/self\.create\(\)/self.create_()/g' "$generated"
done

# Keep checked-in generated code consistent with the repository's Rust style.
"$rustfmt_bin" "$generated_root/mod.rs" \
    "$generated_root/search_kernel/wire/"*_generated.rs \
    "$generated_root/search_kernel/protocol/"*_generated.rs

if [[ "$generated_root" == "$repo_root/core/crates/searchkernel-protocol/src/generated" ]]; then
    "$rustfmt_bin" "$repo_root/core/crates/searchkernel-protocol/src/lib.rs" \
        "$repo_root/core/crates/searchkernel-protocol/src/protocol.rs"
fi
