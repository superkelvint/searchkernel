#!/usr/bin/env bash
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=${1:-"$HERE/libsearchkernel_c7_shim.so"}
mkdir -p "$(dirname "$OUT")"
cc -std=c11 -fPIC -shared -Wall -Wextra -Werror \
  -I"$HERE" "$HERE/c7_ffi_shim.c" -o "$OUT"
printf '%s\n' "$OUT"
