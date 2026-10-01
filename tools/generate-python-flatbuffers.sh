#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$repo_root/tools/resolve-flatbuffers.sh" flatc)
output=${PYTHON_FLATBUFFERS_GENERATED_DIR:-"$repo_root/clients/python/searchkernel/_wire/v1/generated"}
python=$(bash "$repo_root/tools/resolve-python.sh")
mkdir -p "$output"
rm -rf "$repo_root/clients/python/searchkernel/_wire_generated"
find "$output" -type f -delete
find "$output" -depth -type d -empty -delete

"$flatc_bin" \
    --python \
    --python-typing \
    --python-decode-obj-api-strings \
    --gen-object-api \
    -I "$repo_root/api/wire/v1" \
    -o "$output" \
    "$repo_root/api/wire/v1/searchkernel.fbs" \
    "$repo_root/api/wire/v1/searchkernel_request.fbs" \
    "$repo_root/api/wire/v1/searchkernel_response.fbs"

PYTHONPATH="$repo_root${PYTHONPATH:+:$PYTHONPATH}" \
    "$python" "$repo_root/tools/patch-python-flatbuffers-object-api.py" "$output"
"$python" "$repo_root/tools/normalize-generated-text.py" "$output"
"$python" "$repo_root/tools/generate-python-wire-metadata.py" \
    "$repo_root/core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs" \
    "$output/semantic_metadata.py"

