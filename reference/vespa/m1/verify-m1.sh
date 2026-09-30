#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$ROOT/common/verify-common.sh"

"$ROOT/m0/verify-m0.sh"

TARGET=$REPO_ROOT/target/vespa-verifier/m1
CONFIG="$SDK/share/embedded_crud_cfg"
mkdir -p "$TARGET"

cargo check --quiet -p searchkernel-vespa-engine --example api-contract-m1
cargo build --quiet -p searchkernel-vespa-engine --example searchkernel-m1-driver

BIN=$CARGO_TARGET_DIR/debug/examples/searchkernel-m1-driver
if [ ! -x "$BIN" ]; then
    echo "M1 driver binary not found at $BIN" >&2
    exit 1
fi

require_native_driver "$BIN"

rm -rf "$TARGET/data"
"$BIN" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m1/expected/01-rust-open-close.json" \
    "$TARGET/actual.json"

echo "M1 PASS"
