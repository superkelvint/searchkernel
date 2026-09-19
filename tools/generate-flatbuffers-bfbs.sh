#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [[ -n "${FLATC_PATH:-}" ]]; then
    flatc_bin=$FLATC_PATH
elif command -v flatc >/dev/null 2>&1; then
    flatc_bin=$(command -v flatc)
else
    flatc_bin=$(find "$root/target/debug/build" -path '*/flatbuffers-build-*/out/bin/flatc' -type f -print -quit 2>/dev/null || true)
fi
[[ -x "${flatc_bin:-}" ]] || { echo "FlatBuffers 25.12.19 flatc is required" >&2; exit 1; }
[[ "$($flatc_bin --version)" == "flatc version 25.12.19" ]] || { echo "wrong flatc version" >&2; exit 1; }

output="$root/core/crates/searchkernel-json/bfbs"
mkdir -p "$output"
"$flatc_bin" --binary --schema -I "$root/api" -o "$output" "$root/api/searchkernel_request.fbs"
"$flatc_bin" --binary --schema -I "$root/api" -o "$output" "$root/api/searchkernel_response.fbs"
