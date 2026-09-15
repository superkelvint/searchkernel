#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CHECKSUM="$SCRIPT_DIR/M9-IMMUTABLE.sha256"
"$SCRIPT_DIR/../common/freeze-immutable.sh" "$SCRIPT_DIR" "$CHECKSUM" \
  reference/vespa/api-contract/src/bin/m9.rs \
  reference/vespa/m8/M8-IMMUTABLE.sha256 \
  reference/vespa/common/verify-common.sh \
  reference/vespa/common/check-immutable.sh \
  reference/vespa/common/freeze-immutable.sh \
  reference/vespa/common/check-fixture-literals.py
