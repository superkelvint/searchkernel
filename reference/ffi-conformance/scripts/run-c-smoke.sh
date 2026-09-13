#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
root="$(repo_root)"
ref="$root/reference/ffi-conformance"
lib="$(ffi_lib_path "$root")"
[[ -f "$lib" ]] || fail "missing production cdylib: $lib"

build="$root/target/ffi-conformance-c"
mkdir -p "$build"

cc -std=c11 -Wall -Wextra -Werror -pedantic \
  -I"$ref/include" \
  "$ref/c/abi_smoke.c" \
  -L"$root/target/release" -lsearchkernel_ffi \
  -Wl,-rpath,"$root/target/release" \
  -o "$build/abi-smoke"

c++ -std=c++17 -Wall -Wextra -Werror -pedantic \
  -I"$ref/include" \
  "$ref/cpp/header_smoke.cc" \
  -L"$root/target/release" -lsearchkernel_ffi \
  -Wl,-rpath,"$root/target/release" \
  -o "$build/header-smoke"

"$build/abi-smoke"
"$build/header-smoke"
echo "C/C++ ABI caller PASS"
