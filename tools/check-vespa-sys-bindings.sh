#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
EXPECTED=$(mktemp)
trap 'rm -f "$EXPECTED"' EXIT

SK_NATIVE_BINDINGS_OUTPUT="$EXPECTED" \
    cargo check --manifest-path "$ROOT/crates/searchkernel-vespa-sys/Cargo.toml" >/dev/null
diff -u "$ROOT/crates/searchkernel-vespa-sys/src/bindings.rs" "$EXPECTED"
