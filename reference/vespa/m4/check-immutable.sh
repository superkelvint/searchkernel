#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"$SCRIPT_DIR/../common/check-immutable.sh" "$SCRIPT_DIR/M4-IMMUTABLE.sha256"
