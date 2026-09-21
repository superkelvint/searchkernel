#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-$repo_root/target/host}
if [[ "$target_dir" != /* ]]; then target_dir="$repo_root/$target_dir"; fi
expected_version='25.12.19'

if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=$(command -v flatc)
else
    flatc_bin=$(find "$target_dir/debug/build" -path '*/flatbuffers-build-*/out/bin/flatc' -type f -print -quit 2>/dev/null || true)
    if [[ -z "$flatc_bin" && -x /usr/local/bin/flatc ]]; then flatc_bin=/usr/local/bin/flatc; fi
fi

if [[ -z "${flatc_bin:-}" || ! -x "$flatc_bin" ]]; then
    echo "error: FlatBuffers 25.12.19 flatc is required" >&2
    exit 1
fi
if [[ "$($flatc_bin --version)" != "flatc version $expected_version" ]]; then
    echo "error: expected flatc version $expected_version" >&2
    exit 1
fi

output="$repo_root/clients/python/searchkernel/_wire_generated"
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

PYTHONPATH="$repo_root" python3 "$repo_root/tools/patch-python-flatbuffers-object-api.py" "$output"
