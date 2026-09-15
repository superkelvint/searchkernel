#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SK_NATIVE_BINDINGS_CHECK=1 \
    cargo check --manifest-path "$ROOT/core/crates/searchkernel-vespa-bindings/Cargo.toml" >/dev/null
