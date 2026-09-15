#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
MANIFEST="$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml"
BINDINGS="$ROOT/core/crates/searchkernel-vespa-bindings/src/bindings.rs"

if [ "${SK_NATIVE_BINDINGS_REGENERATE:-0}" = 1 ]; then
    SK_NATIVE_BINDINGS_OUTPUT="$BINDINGS" \
    SK_NATIVE_BINDINGS_CHECK=1 \
        cargo check --manifest-path "$MANIFEST"
else
    SK_NATIVE_BINDINGS_CHECK=1 \
        cargo check --manifest-path "$MANIFEST" >/dev/null
fi
