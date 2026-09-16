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
for symbol in sk_create sk_open sk_call sk_close sk_buffer_free; do
  nm "$DRIVER" 2>/dev/null | grep -E "[[:space:]]${symbol}$" >/dev/null ||
    fail_verification "recovery driver does not contain C5 symbol: $symbol"
done

TARGET="$ROOT/target/vespa-verifier/recovery"
rm -rf "$TARGET"
mkdir -p "$TARGET"

LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$DRIVER" matrix "$TARGET/matrix"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$DRIVER" repeat "$TARGET/repeated"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$DRIVER" missing "$TARGET/missing"

echo "cross-boundary recovery PASS"
