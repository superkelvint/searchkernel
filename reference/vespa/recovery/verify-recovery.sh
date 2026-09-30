#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$SCRIPT_DIR/validate.py"
"$SCRIPT_DIR/check-structure.sh"

cargo build --quiet -p searchkernel --features native --example searchkernel-recovery-driver
DRIVER="$CARGO_TARGET_DIR/debug/examples/searchkernel-recovery-driver"
require_native_driver "$DRIVER"

TARGET="$ROOT/target/vespa-verifier/recovery"
rm -rf "$TARGET"
mkdir -p "$TARGET"

"$DRIVER" matrix "$TARGET/matrix"
"$DRIVER" repeat "$TARGET/repeated"
"$DRIVER" missing "$TARGET/missing"

echo "product recovery PASS"
