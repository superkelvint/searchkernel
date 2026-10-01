#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# Wire-major sources are durable identities. The top-level schema paths are
# compatibility symlinks only; api/wire/v1 is the single source of truth.
for schema in searchkernel.fbs searchkernel_request.fbs searchkernel_response.fbs; do
    link="$root/api/$schema"
    expected="wire/v1/$schema"
    if [[ ! -L "$link" || "$(readlink "$link")" != "$expected" ]]; then
        echo "api/$schema must be a symlink to $expected; edit api/wire/v1/$schema instead" >&2
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
