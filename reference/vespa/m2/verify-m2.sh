#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../.." && pwd)
SDK=${SK_NATIVE_SDK:-$REPO_ROOT/native/sdk/linux-x86_64}
TARGET=$REPO_ROOT/target/vespa-verifier/m2
CARGO_TARGET_DIR=$REPO_ROOT/target/vespa-verifier/cargo
export CARGO_TARGET_DIR
CONFIG=$SDK/share/embedded_crud_cfg

mkdir -p "$TARGET"

"$ROOT/common/check-harness-clean.sh"
"$ROOT/common/check-production-shortcuts.sh"

cargo check --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" --bin m2
cargo build --quiet --manifest-path "$ROOT/m2/rust-driver/Cargo.toml"

BIN=$CARGO_TARGET_DIR/debug/searchkernel-m2-driver
if [ ! -x "$BIN" ]; then
    echo "M2 driver binary not found at $BIN" >&2
    exit 1
fi

if ! ldd "$BIN" | grep -q 'libsearchkernel_vespa'; then
    echo "M2 driver is not dynamically linked to libsearchkernel_vespa" >&2
    ldd "$BIN" >&2 || true
    exit 1
fi

# Require the final executable to reference the native operation symbols. This
# is a structural guard against replacing the acceptance path with Rust-only
# behavior.
for symbol in sk_engine_put sk_engine_update sk_engine_remove sk_engine_search; do
    if ! nm -D "$BIN" 2>/dev/null | grep -q "$symbol"; then
        echo "M2 driver does not reference required native symbol: $symbol" >&2
        exit 1
    fi
done

rm -rf "$TARGET/data"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$BIN" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m2/expected/01-crud-attribute-search.json" \
    "$TARGET/actual.json"

echo "M2 PASS"
