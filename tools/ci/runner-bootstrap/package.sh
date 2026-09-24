#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
OUT="${1:-$PWD/searchkernel-actions-runner-bootstrap.zip}"

command -v zip >/dev/null 2>&1 || {
  echo "error: zip is required" >&2
  exit 1
}

rm -f "$OUT"
(
  cd "$SCRIPT_DIR"
  zip -q "$OUT" README.md config.env.example install.sh verify.sh
)

echo "$OUT"
