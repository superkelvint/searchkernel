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

cargo check --quiet -p searchkernel-vespa-engine --example api-contract-m2
cargo build --quiet -p searchkernel-vespa-engine --example searchkernel-m2-driver
BIN=$CARGO_TARGET_DIR/debug/examples/searchkernel-m2-driver
if [ ! -x "$BIN" ]; then
    echo "M2 driver binary not found at $BIN" >&2
    exit 1
fi

require_native_driver "$BIN" sk_engine_put sk_engine_update sk_engine_remove sk_engine_search_generic

rm -rf "$TARGET/data"
cargo run --quiet -p searchkernel-vespa-engine --example searchkernel-m2-driver -- \
    "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m2/expected/01-crud-attribute-search.json" \
    "$TARGET/actual.json"

echo "M2 PASS"
