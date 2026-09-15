#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
OUTPUT="$ROOT/core/crates/searchkernel-ffi/include/searchkernel_ffi.h"
REFERENCE="$ROOT/reference/ffi-conformance/include/searchkernel_ffi.h"

command -v cbindgen >/dev/null 2>&1 || {
  echo "cbindgen is required to regenerate $OUTPUT" >&2
  exit 1
}

cbindgen \
  --config "$ROOT/core/crates/searchkernel-ffi/cbindgen.toml" \
  "$ROOT/core/crates/searchkernel-ffi" \
  --output "$OUTPUT"

cmp -s "$OUTPUT" "$REFERENCE" || {
  diff -u "$REFERENCE" "$OUTPUT" || true
  echo "the normative C5 header must be updated with this generated header" >&2
  exit 1
}
