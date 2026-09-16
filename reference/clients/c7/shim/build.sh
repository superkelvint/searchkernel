#!/usr/bin/env bash
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=${1:-"$HERE/libsearchkernel_c7_shim.so"}
CC=${CC:-cc}
"$CC" -std=c11 -Wall -Wextra -Werror -fPIC -shared \
  "$HERE/c7_ffi_shim.c" -o "$OUT"
printf '%s\n' "$OUT"
