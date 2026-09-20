#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=$(command -v flatc)
else
    flatc_bin=/usr/local/bin/flatc
fi

[[ -x "${flatc_bin:-}" ]] || { echo "FlatBuffers 25.12.19 flatc is required" >&2; exit 1; }
[[ "$($flatc_bin --version)" == "flatc version 25.12.19" ]] || { echo "wrong flatc version" >&2; exit 1; }

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
"$flatc_bin" --binary --schema -I "$root/api" -o "$temporary" "$root/api/searchkernel_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api" -o "$temporary" "$root/api/searchkernel_response.fbs"
cmp "$temporary/searchkernel_request.bfbs" "$root/core/crates/searchkernel-protocol/bfbs/searchkernel_request.bfbs"
cmp "$temporary/searchkernel_response.bfbs" "$root/core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs"
echo "FlatBuffers BFBS reproducibility PASS"
