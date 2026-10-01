#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Wire-major sources are durable identities. The top-level schema files remain
# compatibility mirrors for tooling that has not yet moved under api/wire/v1.
# Fail closed if an edit changes only one side.
for schema in searchkernel.fbs searchkernel_request.fbs searchkernel_response.fbs; do
    if ! cmp -s "$root/api/$schema" "$root/api/wire/v1/$schema"; then
        echo "wire V1 schema drift: api/$schema must mirror api/wire/v1/$schema" >&2
        exit 1
    fi
done
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
