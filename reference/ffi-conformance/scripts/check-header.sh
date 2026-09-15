#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
ref="$root/reference/ffi-conformance/include/searchkernel_ffi.h"
impl="$root/core/crates/searchkernel-ffi/include/searchkernel_ffi.h"
[[ -f "$impl" ]] || fail "missing $impl"
cmp -s "$ref" "$impl" || {
  diff -u "$ref" "$impl" || true
  fail "production C header differs from checked-in ABI header"
}
echo "header contract PASS"
