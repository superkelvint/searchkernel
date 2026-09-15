#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
cd "$root"
command -v cargo >/dev/null 2>&1 || fail "cargo not found"

tree="$(cargo tree -p searchkernel-ffi -e normal --prefix none)"
for forbidden in searchkernel-vespa-engine searchkernel-vespa-bindings searchkernel-vespa-backend searchkernel-testkit; do
  if printf '%s\n' "$tree" | grep -Eq "^${forbidden}([[:space:]]|$)"; then
    fail "normal dependency graph contains forbidden crate: $forbidden"
  fi
done

if grep -Eq '^default[[:space:]]*=.*ffi-test-support' "$root/core/crates/searchkernel-ffi/Cargo.toml"; then
  fail "ffi-test-support must not be a default feature"
fi

echo "dependency boundary PASS"
