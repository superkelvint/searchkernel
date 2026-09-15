#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
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

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
"$flatc_bin" --binary --schema -I "$root/api" -o "$temporary" \
    "$root/api/searchkernel_ffi_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api" -o "$temporary" \
    "$root/api/searchkernel_ffi_response.fbs"

cmp "$temporary/searchkernel_ffi_request.bfbs" \
    "$root/core/crates/searchkernel-json/bfbs/searchkernel_ffi_request.bfbs"
cmp "$temporary/searchkernel_ffi_response.bfbs" \
    "$root/core/crates/searchkernel-json/bfbs/searchkernel_ffi_response.bfbs"
