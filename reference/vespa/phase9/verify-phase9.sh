#!/usr/bin/env bash
set -euo pipefail
PHASE9="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$PHASE9/../../.." && pwd)"
cd "$ROOT"
export SK_NATIVE_SDK="${SK_NATIVE_SDK:-$ROOT/target/vespa-verifier/phase9/sdk}"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
if [[ ! -f "$SDK/lib/libsearchkernel_vespa.so" ]]; then
  SK_NATIVE_REGENERATE_BINDINGS=0 \
    "$ROOT/core/native/vespa/build-sdk.sh"
fi
require_sdk_manifest "$SDK/manifest.json"

# Keep the cumulative gates in the phase9 acceptance path. The extra library
# directories are needed when a pinned-Vespa oracle was built outside its
# original container image.
VESPA_LIBS="$(find "$ROOT/upstream/vespa" -type d \( -path '*/src/vespa/*' -o -path '*/testlib' -o -path '*/test' -o -path '*/features' -o -path '*/memoryindex' \) -print | paste -sd: -)"
VESPA_DEPS_LIB="${VESPA_DEPS_LIB_DIR:-$ROOT/vespa-deps/lib64}"
export LD_LIBRARY_PATH="$SDK/lib${VESPA_DEPS_LIB:+:$VESPA_DEPS_LIB}${VESPA_LIBS:+:$VESPA_LIBS}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

"$ROOT/reference/vespa/verify-through.sh" m13
"$ROOT/reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh"
"$PHASE9/check-structure.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cc -std=c11 -Wall -Wextra -Werror -I"$SDK/include" \
  "$PHASE9/native-probe/probe.c" -L"$SDK/lib" -lsearchkernel_vespa \
  -Wl,-rpath,"$SDK/lib" -o "$TMP/probe"
actual="$("$TMP/probe")"
expected="$(cat "$PHASE9/expected/01-additive-handles.json")"
python3 - "$actual" "$expected" <<'PY'
import json
import sys

actual = json.loads(sys.argv[1])
expected = json.load(open(sys.argv[2], encoding="utf-8"))
if actual != expected:
    raise SystemExit(f"phase9 native probe mismatch: {actual!r} != {expected!r}")
PY

# These executable vertical slices cover the Phase 9 semantics that cannot be
# established by the ABI probe alone: real Vespa arithmetic persistence,
# grouping result decoding/filter serialization, and selected typed-query
# execution. The broader adapter `typed_query` test filter currently contains
# thirteen tests; this cumulative verifier intentionally selects the five
# real-engine slices required by the Phase 9 contract.
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_arithmetic_updates_survive_read_and_reopen
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_nearest_neighbor_options_accept_zero_explore_additional_hits
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_grouping_decodes_quantiles_and_summary_hits
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_grouping_filter_survives_serialization_and_filters_groups
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_raw_sort_spec_is_available_through_the_safe_request_wrapper
cargo test -p searchkernel-vespa-engine --test native_crud \
  native_in_supports_string_and_integer_values_without_approximating_invalid_input
cargo test -p searchkernel-ir-adapter --lib \
  typed_query_a_reaches_real_vespa_through_the_adapter
cargo test -p searchkernel-ir-adapter --lib \
  typed_query_c_special_fields_reach_real_vespa_through_the_adapter
cargo test -p searchkernel-ir-adapter --lib \
  typed_query_c_same_element_reaches_real_vespa_for_arrays_and_relative_members
cargo test -p searchkernel-ir-adapter --lib \
  typed_query_c_geo_reaches_real_vespa_for_location_and_bounding_box
cargo test -p searchkernel-ir-adapter --lib \
  typed_query_c_nearest_neighbor_reaches_real_vespa_through_the_adapter

echo 'phase9 additive native ABI verification passed'
