#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"$SCRIPT_DIR/../common/freeze-immutable.sh" \
  "$SCRIPT_DIR" "$SCRIPT_DIR/M4-IMMUTABLE.sha256" \
  reference/vespa/common/verify-common.sh \
  reference/vespa/common/check-immutable.sh \
  reference/vespa/common/freeze-immutable.sh \
  reference/vespa/common/check-fixture-literals.py \
  reference/vespa/verify-through.sh
