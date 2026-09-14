#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
root=$(c6_root); flatc=$(find_flatc); version=$(flatc_version "$flatc")
[[ "$version" == "25.12.19" ]] || { echo "wrong flatc version: $version" >&2; exit 1; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
generate_bfbs "$flatc" "$root/api" "$tmp/bfbs"
for name in searchkernel_ffi_request.bfbs searchkernel_ffi_response.bfbs; do
  committed="$root/crates/searchkernel-json/bfbs/$name"; regenerated="$tmp/bfbs/$name"
  [[ -f "$regenerated" ]] || { echo "flatc did not generate $name" >&2; exit 1; }
  cmp -s "$committed" "$regenerated" || { echo "committed BFBS drift: crates/searchkernel-json/bfbs/$name" >&2; exit 1; }
done
echo "C6 BFBS provenance PASS ($flatc, $version)"
