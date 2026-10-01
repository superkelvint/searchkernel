#!/usr/bin/env bash
set -euo pipefail
M13="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M13/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
if [[ -n "${FLATC_PATH:-}" ]]; then
  FLATC="$FLATC_PATH"
elif command -v flatc >/dev/null 2>&1; then
  FLATC="$(command -v flatc)"
else
  FLATC="$(find "$ROOT/target" -type f -path '*/bin/flatc' -perm -111 -print -quit 2>/dev/null || true)"
fi
[[ -x "$FLATC" ]] || fail_verification "flatc is required"
out="${1:-$M13/fixture/generated}"
mkdir -p "$out"
"$FLATC" --binary --raw-binary --root-type SearchKernel.Wire.IndexDefinition -o "$out" \
  "$ROOT/api/searchkernel.fbs" "$M13/fixture/index-definition.json"
file="$(find "$out" -maxdepth 1 -type f \( -name '*.bin' -o -name '*.bfbs' \) | head -1)"
[[ -n "$file" ]] || fail_verification "flatc did not produce an IndexDefinition binary"
printf '%s\n' "$file"
