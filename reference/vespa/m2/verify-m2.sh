#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$ROOT/common/verify-common.sh"

"$ROOT/m1/verify-m1.sh"

TARGET=$REPO_ROOT/target/vespa-verifier/m2
CONFIG="$SDK/share/embedded_crud_cfg"
mkdir -p "$TARGET"

check_fixture_literals "$ROOT/m2/scenarios/01-crud-attribute-search.json" \
    "$REPO_ROOT/core/crates/searchkernel-vespa-engine/src" \
    "$REPO_ROOT/core/crates/searchkernel-vespa-bindings/src" "$REPO_ROOT/core/native/vespa"

cargo check --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" --bin m2
cargo build --quiet --manifest-path "$ROOT/m2/rust-driver/Cargo.toml"

BIN=$CARGO_TARGET_DIR/debug/searchkernel-m2-driver
if [ ! -x "$BIN" ]; then
    echo "M2 driver binary not found at $BIN" >&2
    exit 1
fi

require_native_driver "$BIN" sk_engine_put sk_engine_update sk_engine_remove sk_engine_search

rm -rf "$TARGET/data"
run_native_executable "$BIN" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m2/expected/01-crud-attribute-search.json" \
    "$TARGET/actual.json"

echo "M2 PASS"
