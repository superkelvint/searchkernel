#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

SEARCHKERNEL_RUST_GENERATED_DIR="$temporary/generated"   bash "$root/tools/generate-searchkernel-protocol.sh"

diff -ruN --   "$root/core/crates/searchkernel-protocol/src/generated"   "$temporary/generated"

echo "Rust FlatBuffers generation reproducibility PASS"
