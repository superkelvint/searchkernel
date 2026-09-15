#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CHECKSUM="$SCRIPT_DIR/M8-IMMUTABLE.sha256"
"$SCRIPT_DIR/../common/freeze-immutable.sh" "$SCRIPT_DIR" "$CHECKSUM" \
  reference/vespa/api-contract/src/bin/m8.rs \
  reference/vespa/common/verify-common.sh \
  reference/vespa/common/check-immutable.sh \
  reference/vespa/common/freeze-immutable.sh \
  reference/vespa/common/check-fixture-literals.py \
  reference/vespa/m7/M7-IMMUTABLE.sha256
