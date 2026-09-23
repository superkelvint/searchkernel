#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

rust_generated="$temporary/rust"
python_generated="$temporary/python"

SEARCHKERNEL_PROTOCOL_GENERATED_DIR="$rust_generated" \
    bash "$root/tools/generate-searchkernel-protocol.sh" >/dev/null
diff -ruN -- "$root/core/crates/searchkernel-protocol/src/generated" "$rust_generated"

PYTHON_FLATBUFFERS_GENERATED_DIR="$python_generated" \
    bash "$root/tools/generate-python-flatbuffers.sh" >/dev/null
diff -ruN --exclude='__pycache__' --exclude='*.pyc' -- \
    "$root/clients/python/searchkernel/_wire_generated" "$python_generated"

echo "Rust and Python FlatBuffers generation reproducibility PASS"
