#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
lib="$(ffi_lib_path "$root")"
[[ -f "$lib" ]] || fail "missing production cdylib: $lib"

case "$(uname -s)" in
  Linux)
    command -v readelf >/dev/null 2>&1 || fail "readelf not found"
    deps="$(readelf -d "$lib" | grep NEEDED || true)"
    ;;
  Darwin)
    command -v otool >/dev/null 2>&1 || fail "otool not found"
    deps="$(otool -L "$lib" || true)"
    ;;
  *) fail "unsupported verifier host" ;;
esac

if printf '%s\n' "$deps" | grep -Eiq '(vespa|searchcore|proton|searchlib)'; then
  printf '%s\n' "$deps" >&2
  fail "production FFI library links forbidden Vespa/native search libraries"
fi

echo "native dependency boundary PASS"
