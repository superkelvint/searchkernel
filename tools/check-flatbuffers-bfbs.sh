#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$root/tools/resolve-flatbuffers.sh" flatc)
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
"$flatc_bin" --binary --schema -I "$root/api/wire/v1" -o "$temporary" "$root/api/wire/v1/searchkernel_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api/wire/v1" -o "$temporary" "$root/api/wire/v1/searchkernel_response.fbs"
cmp "$temporary/searchkernel_request.bfbs" "$root/core/crates/searchkernel-protocol/bfbs/searchkernel_request.bfbs"
cmp "$temporary/searchkernel_response.bfbs" "$root/core/crates/searchkernel-protocol/bfbs/searchkernel_response.bfbs"
echo "FlatBuffers BFBS reproducibility PASS"
