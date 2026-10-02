#!/usr/bin/env bash
set -euo pipefail
M12="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M12/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
FLATC="${FLATC_PATH:-flatc}"
command -v "$FLATC" >/dev/null 2>&1 || fail_verification "flatc is required"
[[ -f "$M12/fixture/index-definition.json" ]] || fail_verification "missing M12 portable fixture definition"
out="${1:-$M12/fixture/generated}"
mkdir -p "$out"
"$FLATC" --binary --root-type SearchKernel.Wire.IndexDefinition -o "$out" \
  "$ROOT/api/searchkernel.fbs" -- "$M12/fixture/index-definition.json"
file="$(find "$out" -maxdepth 1 -type f -name '*.bin' -o -name '*.bfbs' | head -1)"
[[ -n "$file" ]] || fail_verification "flatc did not produce an IndexDefinition binary"
printf '%s\n' "$file"
