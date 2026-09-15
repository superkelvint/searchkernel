#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
output="$root/core/crates/searchkernel-json/bfbs"

if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=$(command -v flatc)
else
    echo "FlatBuffers 25.12.19 flatc is required; install the official release or set FLATC_PATH" >&2
    exit 1
fi

if [[ ! -x "$flatc_bin" ]]; then
    echo "FlatBuffers flatc is not executable: $flatc_bin" >&2
    exit 1
fi
if [[ "$($flatc_bin --version)" != "flatc version 25.12.19" ]]; then
    echo "expected flatc version 25.12.19" >&2
    exit 1
fi

mkdir -p "$output"
"$flatc_bin" --binary --schema -I "$root/api" -o "$output" \
    "$root/api/searchkernel_ffi_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api" -o "$output" \
    "$root/api/searchkernel_ffi_response.fbs"
