#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
flatc_bin=$(bash "$root/tools/resolve-flatbuffers.sh" flatc)
output="$root/core/crates/searchkernel-protocol/bfbs"
mkdir -p "$output"
"$flatc_bin" --binary --schema -I "$root/api/wire/v1" -o "$output" "$root/api/wire/v1/searchkernel_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api/wire/v1" -o "$output" "$root/api/wire/v1/searchkernel_response.fbs"
