#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
src="$root/crates/searchkernel-ffi/src"
[[ -d "$src" ]] || fail "missing $src"

if grep -RInE --include='*.rs' '(root_unchecked|get_root_unchecked|root_as_.*unchecked)' "$src"; then
  fail "unchecked FlatBuffers parsing is forbidden in searchkernel-ffi"
fi

if grep -RInE --include='*.rs' 'searchkernel[_-]vespa' "$src" "$root/crates/searchkernel-ffi/Cargo.toml" "$root/crates/searchkernel-ffi/build.rs" 2>/dev/null; then
  fail "Vespa dependency/reference found in searchkernel-ffi"
fi

# The test-support module is allowed to mention the testkit. Production dispatch is not.
if grep -RInE --include='*.rs' 'searchkernel_testkit' "$src" | grep -v '/test_support.rs:'; then
  fail "searchkernel-testkit may only be referenced from feature-gated test_support.rs"
fi

# Keep the production ABI tiny even if an implementation accidentally adds no_mangle exports.
if grep -RInE --include='*.rs' 'fn[[:space:]]+sk_(insert|upsert|patch|delete|get|search|analyze|filter|facet|field|rank)' "$src"; then
  fail "per-operation C ABI function found"
fi

echo "source invariants PASS"
