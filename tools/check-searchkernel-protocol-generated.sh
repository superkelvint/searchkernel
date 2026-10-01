#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

SEARCHKERNEL_RUST_GENERATED_DIR="$temporary/generated"   bash "$root/tools/generate-searchkernel-protocol.sh"

diff -ruN --   "$root/core/crates/searchkernel-protocol/src/generated"   "$temporary/generated"

protocol_module="$root/core/crates/searchkernel-protocol/src/protocol.rs"
for generated in "$root/core/crates/searchkernel-protocol/src/generated/search_kernel/protocol/"*_generated.rs; do
    basename=$(basename "$generated")
    if ! grep -Fq "include!(\"generated/search_kernel/protocol/$basename\");" "$protocol_module"; then
        echo "Rust protocol module is missing generated export: $basename" >&2
        exit 1
    fi
done

echo "Rust FlatBuffers generation reproducibility PASS"
