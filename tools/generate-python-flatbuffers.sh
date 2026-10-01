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
    -I "$repo_root/api" \
    -o "$output" \
    "$repo_root/api/searchkernel.fbs" \
    "$repo_root/api/searchkernel_request.fbs" \
    "$repo_root/api/searchkernel_response.fbs"

PYTHONPATH="$repo_root${PYTHONPATH:+:$PYTHONPATH}" \
    "$python" "$repo_root/tools/patch-python-flatbuffers-object-api.py" "$output"
"$python" "$repo_root/tools/normalize-generated-text.py" "$output"

PYTHONPATH="$repo_root/clients/python${PYTHONPATH:+:$PYTHONPATH}" \
    "$python" "$repo_root/tools/regenerate-request-fixtures.py" \
    "$repo_root/core/crates/searchkernel-dispatch/tests/fixtures/portable/requests"
PYTHONPATH="$repo_root/clients/python${PYTHONPATH:+:$PYTHONPATH}" \
    "$python" "$repo_root/tools/regenerate-response-error-fixtures.py" \
    "$repo_root/core/crates/searchkernel-dispatch/tests/fixtures/portable/responses"
