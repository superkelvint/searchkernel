#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
lib="$(ffi_lib_path "$root")"
[[ -f "$lib" ]] || fail "missing production cdylib: $lib"
command -v nm >/dev/null 2>&1 || fail "nm not found"

case "$(uname -s)" in
  Linux)
    symbols="$(nm -D --defined-only "$lib" 2>/dev/null | awk '{print $NF}')"
    ;;
  Darwin)
    symbols="$(nm -gU "$lib" 2>/dev/null | awk '{print $NF}' | sed 's/^_//')"
    ;;
  *) fail "unsupported verifier host" ;;
esac

actual="$(printf '%s\n' "$symbols" | grep '^sk_' | LC_ALL=C sort -u || true)"
expected="$(printf '%s\n' sk_abi_version sk_buffer_free sk_call sk_close | LC_ALL=C sort)"
if [[ "$actual" != "$expected" ]]; then
  echo "expected sk_* symbols:" >&2
  printf '%s\n' "$expected" >&2
  echo "actual sk_* symbols:" >&2
  printf '%s\n' "$actual" >&2
  fail "production ABI symbol set differs"
fi

echo "production symbol surface PASS"
