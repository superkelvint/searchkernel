#!/usr/bin/env bash
set -euo pipefail

repo_root() {
  local here
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
  printf '%s\n' "$here"
}

ffi_lib_path() {
  local root="$1"
  case "$(uname -s)" in
    Linux)  printf '%s\n' "$root/target/release/libsearchkernel_ffi.so" ;;
    Darwin) printf '%s\n' "$root/target/release/libsearchkernel_ffi.dylib" ;;
    *) echo "unsupported verifier host: $(uname -s)" >&2; return 1 ;;
  esac
}

fail() {
  echo "ffi-conformance: FAIL: $*" >&2
  exit 1
}
