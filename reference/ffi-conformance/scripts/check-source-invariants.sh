#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
src="$root/core/crates/searchkernel-ffi/src"
[[ -d "$src" ]] || fail "missing $src"

if grep -RInE --include='*.rs' '(root_unchecked|get_root_unchecked|root_as_.*unchecked)' "$src"; then
  fail "unchecked FlatBuffers parsing is forbidden in searchkernel-ffi"
fi

if grep -RInE --include='*.rs' 'searchkernel[_-]vespa' "$src" "$root/core/crates/searchkernel-ffi/Cargo.toml" "$root/core/crates/searchkernel-ffi/build.rs" 2>/dev/null; then
  fail "Vespa dependency/reference found in searchkernel-ffi"
fi

# Feature-gated test-only modules are allowed to mention the testkit. Production
# dispatch is not.
if grep -RInE --include='*.rs' 'searchkernel_testkit' "$src" \
    | grep -v -E '/(test_support|json_convergence_tests)\.rs:'; then
  fail "searchkernel-testkit may only be referenced from feature-gated test-only modules"
fi

# Keep the production ABI tiny even if an implementation accidentally adds no_mangle exports.
if grep -RInE --include='*.rs' 'fn[[:space:]]+sk_(insert|upsert|patch|delete|get|search|analyze|filter|facet|field|rank)' "$src"; then
  fail "per-operation C ABI function found"
fi

echo "source invariants PASS"
