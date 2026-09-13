#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: freeze-immutable.sh SCOPE CHECKSUM [EXTRA_FILE ...]" >&2
  exit 2
fi

COMMON_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$COMMON_DIR/../../.." && pwd)"
SCOPE="$1"
CHECKSUM="$2"
shift 2
SCOPE_REL="${SCOPE#"$ROOT/"}"

{
  find "$SCOPE_REL" -type f \
    ! -path '*/target/*' \
    ! -path '*/build/*' \
    ! -path '*/__pycache__/*' \
    ! -name "$(basename "$CHECKSUM")" \
    -print
  printf '%s\n' "$@"
} | (
  cd "$ROOT"
  sort -u | xargs sha256sum
) > "$CHECKSUM"
