#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../.." && pwd)"
HEADER="$ROOT/core/native/vespa/searchkernel_vespa.h"
MAP="$ROOT/core/native/vespa/searchkernel_vespa.map"
VESPA="$ROOT/upstream/vespa"

trace_sources=(
  searchlib/src/vespa/searchlib/aggregation/group.h
  searchlib/src/vespa/searchlib/aggregation/group.cpp
  searchlib/src/vespa/searchlib/aggregation/grouping.h
  searchlib/src/vespa/searchlib/aggregation/grouping.cpp
  searchlib/src/vespa/searchlib/aggregation/groupinglevel.h
  searchlib/src/vespa/searchlib/aggregation/groupinglevel.cpp
  searchlib/src/vespa/searchlib/aggregation/aggregationresult.h
  searchlib/src/vespa/searchlib/aggregation/hitsaggregationresult.h
  searchlib/src/vespa/searchlib/aggregation/hitsaggregationresult.cpp
  searchlib/src/vespa/searchlib/aggregation/countaggregationresult.h
  searchlib/src/vespa/searchlib/aggregation/countaggregationresult.cpp
  searchlib/src/vespa/searchlib/aggregation/averageaggregationresult.h
  searchlib/src/vespa/searchlib/aggregation/averageaggregationresult.cpp
  searchlib/src/vespa/searchlib/expression/expressionnode.h
  searchlib/src/vespa/searchlib/expression/attributenode.h
  searchlib/src/vespa/searchlib/expression/attributenode.cpp
  searchlib/src/vespa/searchlib/expression/resultnode.h
  searchlib/src/vespa/searchlib/expression/resultnode.cpp
  searchcore/src/vespa/searchcore/proton/matching/matcher.h
  searchcore/src/vespa/searchcore/proton/matching/matcher.cpp
)

for source in "${trace_sources[@]}"; do
  [[ -f "$VESPA/$source" ]] || {
    echo "missing pinned Vespa source-trace file: $source" >&2
    exit 1
  }
done

symbols=(
  sk_grouping_plan_create
  sk_grouping_expression_attribute
  sk_grouping_expression_destroy
  sk_grouping_aggregation_count
  sk_grouping_aggregation_average
  sk_grouping_aggregation_destroy
  sk_grouping_plan_add_level
  sk_grouping_level_add_count
  sk_grouping_level_add_aggregation
  sk_grouping_level_add_aggregation_handle
  sk_grouping_plan_destroy
  sk_engine_group_tree
  sk_grouping_tree_result_root
  sk_grouping_tree_node_group_count
  sk_grouping_tree_node_group
  sk_grouping_tree_node_key
  sk_grouping_result_value_kind
  sk_grouping_result_value_string
  sk_grouping_result_value_integer
  sk_grouping_result_value_float
  sk_grouping_tree_node_output_count
  sk_grouping_tree_node_output
  sk_grouping_tree_output_kind
  sk_grouping_tree_output_value
  sk_grouping_tree_result_destroy
)

for symbol in "${symbols[@]}"; do
  grep -q "$symbol" "$HEADER" || { echo "missing ABI declaration: $symbol" >&2; exit 1; }
  grep -q "$symbol" "$MAP" || { echo "missing exported ABI symbol: $symbol" >&2; exit 1; }
done

if grep -Eq 'sk_grouping_(all|each)' "$HEADER" "$MAP"; then
  echo "grouping container syntax must not become native ABI nodes" >&2
  exit 1
fi

# The new low-level native layer must stay independent of the C-series IR.
NATIVE_CPP="${SK_GROUPING_NATIVE_CPP:-$ROOT/core/native/vespa/engine.cc:$ROOT/core/native/vespa/grouping.cc}"

native_firewall_paths=("$ROOT/core/native/vespa")
if [[ -n "${SK_GROUPING_NATIVE_CPP:-}" ]]; then
  IFS=: read -r -a native_override_paths <<< "$SK_GROUPING_NATIVE_CPP"
  native_firewall_paths+=("${native_override_paths[@]}")
fi

if grep -R --line-number \
    --include='*.cc' --include='*.cpp' --include='*.h' --include='*.hpp' \
    'searchkernel-ir\|GroupingRequestIr' \
    "${native_firewall_paths[@]}" >/dev/null; then
  echo "native grouping implementation depends on C-series IR terminology" >&2
  exit 1
fi

# The general grouping path is spread across the engine lowering/execution
# helpers and the recursive result decoders. Extract those concrete bodies and
# inspect only executable source: comments and string literals are masked before
# matching so documentation cannot satisfy this anti-shortcut verifier.
python3 "reference/vespa/m9-general-grouping-verifier/check-structure-python-1.py" "$NATIVE_CPP"

echo "recursive grouping ABI structure checks passed"
