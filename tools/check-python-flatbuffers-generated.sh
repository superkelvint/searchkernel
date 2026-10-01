#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

PYTHON_FLATBUFFERS_GENERATED_DIR="$temporary/generated"   bash "$root/tools/generate-python-flatbuffers.sh"

diff -ruNB --exclude='__pycache__' --exclude='*.pyc' \
    "$root/clients/python/searchkernel/_wire/v1/generated" \
    "$temporary/generated"

echo "Python FlatBuffers generation reproducibility PASS"
