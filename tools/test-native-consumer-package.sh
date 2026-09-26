#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$ROOT"
python3 tools/generate-native-consumer-bundle.py --check
cargo package -p searchkernel-vespa-bindings --allow-dirty --no-verify >/dev/null
CRATE=$(find "${CARGO_TARGET_DIR:-target}" -path '*/package/searchkernel-vespa-bindings-*.crate' -print | head -1)
[[ -n "$CRATE" && -f "$CRATE" ]] || { echo "packaged bindings crate not found" >&2; exit 1; }
tar -xf "$CRATE" -C "$TMP"
PKG=$(find "$TMP" -mindepth 1 -maxdepth 1 -type d -name 'searchkernel-vespa-bindings-*' -print -quit)
[[ -n "$PKG" ]] || { echo "extracted bindings package not found" >&2; exit 1; }
CONSUMER="$TMP/consumer"
mkdir -p "$CONSUMER/src"
cat > "$CONSUMER/Cargo.toml" <<EOF
[package]
name = "searchkernel-native-consumer"
version = "0.1.0"
edition = "2021"

[dependencies]
searchkernel-vespa-bindings = { path = "$PKG", features = ["native"] }
EOF
cat > "$CONSUMER/src/main.rs" <<'EOF'
fn main() {
    let abi = unsafe { searchkernel_vespa_bindings::sk_native_abi_version() };
    assert_eq!(abi, searchkernel_vespa_bindings::EXPECTED_ABI);
}
EOF
(
  cd "$CONSUMER"
  env -u LD_LIBRARY_PATH -u SK_NATIVE_SDK -u SK_NATIVE_CACHE_DIR \
    -u SK_NATIVE_DISABLE_PUBLISHED_BASE XDG_CACHE_HOME="$TMP/cache" cargo build
)
BIN="$CONSUMER/target/debug/searchkernel-native-consumer"
readelf -d "$BIN" | grep -Fq '\$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2' || {
  echo "packaged Rust consumer lacks self-resolving Search Kernel adapter DT_NEEDED entry" >&2
  readelf -d "$BIN" >&2
  exit 1
}
[[ -f "$CONSUMER/target/debug/.searchkernel-native/libsearchkernel_vespa.so.2" ]] || {
  echo "packaged Rust consumer did not stage its native adapter beside the executable" >&2
  exit 1
}
(
  cd /
  env -u LD_LIBRARY_PATH "$BIN"
)
echo "Packaged native consumer PASS: external Rust binary links and runs without LD_LIBRARY_PATH or repository Cargo config"
