#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$repo_root/tools/resolve-flatc.sh")
output=${PYTHON_FLATBUFFERS_GENERATED_DIR:-"$repo_root/clients/python/searchkernel/_wire_generated"}
python=${PYTHON:-python3}
mkdir -p "$output"
find "$output" -type f -delete
find "$output" -depth -type d -empty -delete

"$flatc_bin" \
    --python \
    --python-typing \
    --python-decode-obj-api-strings \
    --gen-object-api \
    -I "$repo_root/api" \
    -o "$output" \
    "$repo_root/api/searchkernel.fbs" \
    "$repo_root/api/searchkernel_request.fbs" \
    "$repo_root/api/searchkernel_response.fbs"

PYTHONPATH="$repo_root${PYTHONPATH:+:$PYTHONPATH}" \
    "$python" "$repo_root/tools/patch-python-flatbuffers-object-api.py" "$output"
