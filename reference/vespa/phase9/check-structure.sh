#!/usr/bin/env bash
set -euo pipefail
PHASE9="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$PHASE9/../../.." && pwd)"
SDK="${SK_NATIVE_SDK:-$ROOT/core/native/sdk/linux-x86_64}"
HEADER="$ROOT/core/native/vespa/searchkernel_vespa.h"
MAP="$ROOT/core/native/vespa/searchkernel_vespa.map"
BINDINGS="$ROOT/core/crates/searchkernel-vespa-bindings/src/bindings.rs"
SYMBOLS="$(nm -D --defined-only "$SDK/lib/libsearchkernel_vespa.so.2")"

cmp -s "$HEADER" "$SDK/include/searchkernel_vespa.h"
grep -qx '#define SK_NATIVE_ABI_VERSION 12U' "$HEADER"
for symbol in \
  sk_nearest_neighbor_options_create \
  sk_nearest_neighbor_options_destroy \
  sk_nearest_neighbor_options_set_distance_threshold \
  sk_nearest_neighbor_options_set_explore_additional_hits \
  sk_query_add_nearest_neighbor_with_options_handle \
  sk_query_add_same_element_with_filter \
  sk_struct_array_create sk_struct_array_add_element \
  sk_struct_array_set_string sk_struct_array_set_integer \
  sk_struct_array_set_float sk_struct_array_set_boolean \
  sk_struct_array_destroy sk_document_add_struct_array \
  sk_grouping_level_set_max_groups sk_grouping_level_set_precision \
  sk_grouping_filter_regex sk_grouping_level_set_filter \
  sk_grouping_level_add_order_by \
  sk_grouping_aggregation_sum sk_grouping_aggregation_min \
  sk_grouping_aggregation_max sk_grouping_aggregation_stddev \
  sk_grouping_aggregation_xor sk_grouping_aggregation_quantiles \
  sk_grouping_aggregation_summary \
  sk_grouping_tree_output_hit_count sk_grouping_tree_output_hit_id \
  sk_grouping_tree_output_hit_score \
  sk_grouping_tree_output_hit_summary_count \
  sk_grouping_tree_output_hit_summary_name \
  sk_grouping_tree_output_hit_summary_value \
  sk_document_update_arithmetic sk_generic_search_request_set_sort_spec; do
  grep -q "${symbol}" "$HEADER" "$MAP" "$BINDINGS"
  grep -q " ${symbol}" <<<"$SYMBOLS"
done

if grep -Eiq 'sk_query_add_(prefix|substring|suffix)' "$HEADER" "$MAP"; then
  echo 'phase9 must not invent absent query IR operators' >&2
  exit 1
fi
if grep -Eiq 'fake|precomputed|hard.?coded.*(score|result)' \
    "$ROOT/core/native/vespa"/*.cc; then
  echo 'phase9 native adapter contains a shortcut marker' >&2
  exit 1
fi

python3 - "$ROOT/core/native/vespa" <<'PY'
import pathlib
import re
import sys

source_root = pathlib.Path(sys.argv[1])
unity = (source_root / "abi.cc").read_text(encoding="utf-8")
for source_path in sorted(source_root.glob("*.cc")):
    if source_path.name != "abi.cc" and f'#include "{source_path.name}"' not in unity:
        raise SystemExit(f"native adapter source is not included by abi.cc: {source_path.name}")

source = unity

def body(name):
    match = re.search(r"\b" + re.escape(name) + r"\s*\([^)]*\)\s*(?:SK_NOEXCEPT\s*)?\{", source)
    if not match:
        raise SystemExit(f"missing native function: {name}")
    start = match.end() - 1
    depth = 0
    for index in range(start, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    raise SystemExit(f"unterminated native function: {name}")

for name, native_type in (
    ("sk_query_add_in_strings", "MultiTerm::Type::STRING"),
    ("sk_query_add_in_integers", "MultiTerm::Type::INTEGER"),
):
    function = body(name)
    if "add_in_term" not in function or native_type not in function:
        raise SystemExit(f"{name} does not construct the native Vespa InTerm")
    if "addOr" in function or "addNumberTerm" in function:
        raise SystemExit(f"{name} regressed to an equality disjunction")

if "addWeakAnd" not in body("sk_query_add_weak_and"):
    raise SystemExit("text lowering lost its native WeakAnd constructor")
PY

echo 'phase9 native structure checks passed'
