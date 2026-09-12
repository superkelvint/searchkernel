#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
REL="reference/vespa/m4"
OUT="$M4/M4-IMMUTABLE.sha256"
TMP="$OUT.tmp"
cd "$ROOT"
{
  find "$REL" -type f \
    ! -path '*/target/*' \
    ! -name 'M4-IMMUTABLE.sha256' \
    ! -name 'M4-IMMUTABLE.sha256.tmp' \
    -print | LC_ALL=C sort | xargs sha256sum
  if [[ -f reference/vespa/api-contract/src/bin/m4.rs ]]; then
    sha256sum reference/vespa/api-contract/src/bin/m4.rs
  fi
} > "$TMP"
mv "$TMP" "$OUT"
