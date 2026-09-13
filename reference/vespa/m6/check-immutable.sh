#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
CHECKSUM="$SCRIPT_DIR/M6-IMMUTABLE.sha256"

[[ -s "$CHECKSUM" ]] || {
  echo "missing M6 immutable checksum: $CHECKSUM" >&2
  exit 1
}

cd "$ROOT"
sha256sum --quiet --check "$CHECKSUM"
