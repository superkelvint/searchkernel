#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
PYTHON=$("$ROOT/tools/resolve-python.sh")
CARGO=$("$ROOT/tools/resolve-rust.sh" cargo)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$ROOT"
"$PYTHON" tools/generate-native-consumer-bundle.py --check
CONSUMER="$TMP/consumer"
mkdir -p "$CONSUMER/src"
case "$CONSUMER" in
  "$ROOT"/*)
    echo "external consumer unexpectedly lives inside the Search Kernel repository" >&2
    exit 1
    ;;
esac
cat > "$CONSUMER/Cargo.toml" <<EOF
[package]
name = "searchkernel-native-consumer"
version = "0.1.0"
edition = "2021"

[dependencies]
searchkernel = { path = "$ROOT/core/crates/searchkernel", features = ["native"] }
EOF
cat > "$CONSUMER/src/main.rs" <<'EOF'
fn main() {
    let missing = std::env::temp_dir().join("searchkernel-native-consumer-missing-index");
    assert!(searchkernel::Index::open(missing).is_err());
}
EOF
(
  cd "$CONSUMER"
  env -u LD_LIBRARY_PATH -u SK_NATIVE_SDK -u SK_NATIVE_CACHE_DIR \
    -u SK_NATIVE_DISABLE_PUBLISHED_BASE XDG_CACHE_HOME="$TMP/cache" "$CARGO" build
)
case "${CARGO_TARGET_DIR:-}" in
  /*) CONSUMER_TARGET=$CARGO_TARGET_DIR ;;
  "") CONSUMER_TARGET="$CONSUMER/target" ;;
  *) CONSUMER_TARGET="$CONSUMER/$CARGO_TARGET_DIR" ;;
esac
BIN="$CONSUMER_TARGET/debug/searchkernel-native-consumer"
readelf -d "$BIN" | grep -Fq '$ORIGIN/.searchkernel-native/libsearchkernel_vespa.so.2' || {
  echo "public Search Kernel Rust consumer lacks self-resolving adapter DT_NEEDED entry" >&2
  readelf -d "$BIN" >&2
  exit 1
}
[[ -f "$CONSUMER_TARGET/debug/.searchkernel-native/libsearchkernel_vespa.so.2" ]] || {
  echo "public Search Kernel Rust consumer did not stage its native adapter beside the executable" >&2
  exit 1
}
(
  cd /
  env -u LD_LIBRARY_PATH "$BIN"
)
echo "Packaged native consumer PASS: external public Search Kernel Rust binary links and runs without LD_LIBRARY_PATH or repository Cargo config"
