#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
expected='25.12.19'
flatc_bin=${FLATC_PATH:-}
if [[ -z "$flatc_bin" ]] && command -v flatc >/dev/null 2>&1; then flatc_bin=$(command -v flatc); fi
if [[ -z "$flatc_bin" ]]; then flatc_bin=$(find "$root/target/debug/build" -path '*/flatbuffers-build-*/out/bin/flatc' -type f -print -quit 2>/dev/null || true); fi
if [[ -z "$flatc_bin" || ! -x "$flatc_bin" ]]; then echo 'error: FlatBuffers 25.12.19 flatc is required (set FLATC_PATH)' >&2; exit 1; fi
[[ "$($flatc_bin --version)" == "flatc version $expected" ]] || { echo "error: expected flatc $expected" >&2; exit 1; }
out="$root/clients/typescript/src/generated/flatbuffers"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
"$flatc_bin" --ts --gen-object-api --gen-all -I "$root/api" -o "$tmp" \
  "$root/api/searchkernel.fbs" "$root/api/searchkernel_request.fbs" "$root/api/searchkernel_response.fbs"
rm -rf "$out"
mkdir -p "$(dirname "$out")"
mv "$tmp" "$out"
trap - EXIT
