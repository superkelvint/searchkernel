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
(
  cd "$PKG"
  env -u SK_NATIVE_SDK -u SK_NATIVE_CACHE_DIR -u SK_NATIVE_DISABLE_PUBLISHED_BASE XDG_CACHE_HOME="$TMP/cache" cargo check --features native
)
echo "Packaged native consumer PASS: clean extracted crate provisions and builds without monorepo files"
