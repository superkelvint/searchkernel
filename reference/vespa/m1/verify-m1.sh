#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ROOT/../.." && pwd)
SDK=${SK_NATIVE_SDK:-$REPO_ROOT/native/sdk/linux-x86_64}
TARGET=$REPO_ROOT/target/vespa-verifier/m1
CARGO_TARGET_DIR=$REPO_ROOT/target/vespa-verifier/cargo
export CARGO_TARGET_DIR
CONFIG=$SDK/share/embedded_crud_cfg

mkdir -p "$TARGET"

"$ROOT/common/check-harness-clean.sh"
"$ROOT/common/check-production-shortcuts.sh"

cargo check --quiet --manifest-path "$ROOT/api-contract/Cargo.toml" --bin m1
cargo build --quiet --manifest-path "$ROOT/m1/rust-driver/Cargo.toml"

BIN=$CARGO_TARGET_DIR/debug/searchkernel-m1-driver
if [ ! -x "$BIN" ]; then
    echo "M1 driver binary not found at $BIN" >&2
    exit 1
fi

if ! ldd "$BIN" | grep -q 'libsearchkernel_vespa'; then
    echo "M1 driver is not dynamically linked to libsearchkernel_vespa" >&2
    ldd "$BIN" >&2 || true
    exit 1
fi

rm -rf "$TARGET/data"
LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$BIN" "$TARGET/data" "$CONFIG" > "$TARGET/actual.json"

python3 "$ROOT/common/compare_json.py" \
    "$ROOT/m1/expected/01-rust-open-close.json" \
    "$TARGET/actual.json"

echo "M1 PASS"
