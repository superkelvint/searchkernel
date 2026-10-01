#!/usr/bin/env bash
set -euo pipefail
M14="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M14/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
FLATC="${FLATC_PATH:-flatc}"
command -v "$FLATC" >/dev/null 2>&1 || fail_verification "flatc is required"
out="${1:-$M14/fixture/generated}"
definition_json="${2:-$M14/fixture/index-definition.json}"
mkdir -p "$out"
"$FLATC" --binary --root-type SearchKernel.Wire.IndexDefinition -o "$out" \
  "$ROOT/api/searchkernel.fbs" "$definition_json"
file="$(find "$out" -maxdepth 1 -type f \( -name '*.bin' -o -name '*.bfbs' \) | head -1)"
[[ -n "$file" ]] || fail_verification "flatc did not produce M14 IndexDefinition bytes"
printf '%s\n' "$file"
