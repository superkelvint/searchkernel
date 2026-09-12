#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3_REL="reference/vespa/m3"
M3="$ROOT/$M3_REL"
OUT="$M3/M3-IMMUTABLE.sha256"
TMP="$OUT.tmp"
cd "$ROOT"

{
  find "$M3_REL" -type f \
    ! -path '*/target/*' \
    ! -name 'M3-IMMUTABLE.sha256' \
    ! -name 'M3-IMMUTABLE.sha256.tmp' \
    -print | LC_ALL=C sort | xargs sha256sum
  if [[ -f reference/vespa/api-contract/src/bin/m3.rs ]]; then
    sha256sum reference/vespa/api-contract/src/bin/m3.rs
  fi
} > "$TMP"
mv "$TMP" "$OUT"
