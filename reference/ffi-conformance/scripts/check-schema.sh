#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
for f in searchkernel_ffi_request.fbs searchkernel_ffi_response.fbs; do
  ref="$root/reference/ffi-conformance/schema/$f"
  impl="$root/api/$f"
  [[ -f "$impl" ]] || fail "missing $impl"
  cmp -s "$ref" "$impl" || {
    diff -u "$ref" "$impl" || true
    fail "$f differs from checked-in transport schema"
  }
done
echo "transport schema PASS"
