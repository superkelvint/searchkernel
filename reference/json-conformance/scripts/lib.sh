#!/usr/bin/env bash
set -euo pipefail

c6_here() { cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd; }
c6_root() { local here; here=$(c6_here); cd "$here/../.." && pwd; }

flatc_version() {
  "$1" --version 2>&1 | sed -E 's/^[^0-9]*([0-9]+\.[0-9]+\.[0-9]+).*$/\1/' | head -n 1
}

find_flatc() {
  local root candidate version
  root=$(c6_root)
  if [[ -n "${FLATC_PATH:-}" ]]; then
    candidate=$FLATC_PATH
    [[ -x "$candidate" ]] || { echo "FLATC_PATH is not executable: $candidate" >&2; return 1; }
    version=$(flatc_version "$candidate")
    [[ "$version" == "25.12.19" ]] || { echo "FLATC_PATH must be 25.12.19, got: $version" >&2; return 1; }
    printf '%s\n' "$candidate"; return 0
  fi
  if command -v flatc >/dev/null 2>&1; then
    candidate=$(command -v flatc); version=$(flatc_version "$candidate")
    if [[ "$version" == "25.12.19" ]]; then printf '%s\n' "$candidate"; return 0; fi
  fi
  if [[ -d "$root/target" ]]; then
    while IFS= read -r candidate; do
      [[ -x "$candidate" ]] || continue
      version=$(flatc_version "$candidate" || true)
      if [[ "$version" == "25.12.19" ]]; then printf '%s\n' "$candidate"; return 0; fi
    done < <(find "$root/target" -type f -name flatc -print 2>/dev/null | sort)
  fi
  echo "FlatBuffers flatc 25.12.19 not found. Set FLATC_PATH, put it on PATH, or build the pinned tool under target/." >&2
  return 1
}

generate_bfbs() {
  local flatc=$1 schema_dir=$2 output_dir=$3
  mkdir -p "$output_dir"
  "$flatc" --binary --schema -I "$schema_dir" -o "$output_dir" "$schema_dir/searchkernel_ffi_request.fbs"
  "$flatc" --binary --schema -I "$schema_dir" -o "$output_dir" "$schema_dir/searchkernel_ffi_response.fbs"
}

bindgen_binary() {
  local root target_dir binary
  root=$(c6_root)
  ( cd "$root"; cargo build -q -p searchkernel-bindgen )
  target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  binary="$target_dir/debug/searchkernel-bindgen"
  [[ -x "$binary" ]] || { echo "searchkernel-bindgen executable not found after build: $binary" >&2; return 1; }
  printf '%s\n' "$binary"
}
