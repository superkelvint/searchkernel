#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
CHECKSUM="$SCRIPT_DIR/M6-IMMUTABLE.sha256"

{
  printf '%s\n' reference/vespa/api-contract/src/bin/m6.rs
  find reference/vespa/m6 -type f \
    ! -name M6-IMMUTABLE.sha256 \
    ! -path '*/target/*' \
    ! -path '*/__pycache__/*' \
    -print | sort
} | (
  cd "$ROOT"
  xargs sha256sum
) > "$CHECKSUM"
