#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$SCRIPT_DIR/validate.py"
"$SCRIPT_DIR/check-structure.sh"

DRIVER_TARGET="$CARGO_TARGET_DIR/recovery-driver"
export CARGO_TARGET_DIR="$DRIVER_TARGET"
cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml"
DRIVER="$DRIVER_TARGET/debug/searchkernel-recovery-driver"
require_native_driver "$DRIVER"

TARGET="$ROOT/target/vespa-verifier/recovery"
rm -rf "$TARGET"
mkdir -p "$TARGET"

run_native_executable "$DRIVER" matrix "$TARGET/matrix"
run_native_executable "$DRIVER" repeat "$TARGET/repeated"
run_native_executable "$DRIVER" missing "$TARGET/missing"

echo "product recovery PASS"
