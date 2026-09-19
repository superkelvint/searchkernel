#!/usr/bin/env bash
set -euo pipefail
M13="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M13/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
[[ -f "$ROOT/reference/vespa/m10/feature-values/expected.json" ]] ||
  fail_verification 'M10 feature-values golden is missing; M13 is supplemental only'
grep -q 'FeatureSet::Value' "$M13/CONTRACT.md"
grep -q 'DOUBLE' "$M13/CONTRACT.md"
grep -q 'DATA' "$M13/CONTRACT.md"
grep -q 'sk_feature_value' "$ROOT/core/native/vespa/searchkernel_vespa.h"
grep -q 'SK_FEATURE_VALUE_DATA' "$ROOT/core/native/vespa/searchkernel_vespa.h"
grep -q 'sk_feature_value_data' "$ROOT/core/native/vespa/searchkernel_vespa.h"
if rg -n 'typedef struct sk_value|sk_value' "$ROOT/core/native" "$ROOT/core/crates"; then
  fail_verification 'M13 must not introduce sk_value'
fi

cargo fmt --all --check
cargo build --quiet --manifest-path "$M13/rust-driver/Cargo.toml" --bin searchkernel-m13-driver
DRIVER="$CARGO_TARGET_DIR/debug/searchkernel-m13-driver"
require_native_driver "$DRIVER" \
  sk_feature_value_kind \
  sk_feature_value_double \
  sk_feature_value_data \
  sk_generic_search_result_match_feature_value \
  sk_generic_search_result_match_feature_value_ref

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
"$DRIVER" "$TMP/config" "$TMP/data"
echo 'M13 supplemental smoke verification passed (M10 feature-values is authoritative)'
