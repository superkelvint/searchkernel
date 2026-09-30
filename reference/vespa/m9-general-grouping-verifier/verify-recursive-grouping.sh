#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

prepare_native_probe_sdk
require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi
"$DIR/validate.py"
"$DIR/check-structure.sh"
python3 "$DIR/test-check-structure.py"

SCENARIO="$DIR/scenarios/01-recursive-two-level.json"
EXPECTED="$DIR/expected/recursive-two-level.json"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  [[ -x "$DIR/oracle/target/m9_recursive_grouping_oracle" ]] || \
    fail_verification "direct oracle is missing; build it with oracle/build-cpp-oracle.sh"
  "$DIR/oracle/run-cpp-oracle.sh" "$SCENARIO" \
    | "$DIR/canonicalize.py" > "$TMP/oracle.json"
  python3 "$ROOT/reference/vespa/common/compare_json.py" "$EXPECTED" "$TMP/oracle.json"
fi

# Compile a plain C consumer against the explicitly prepared native-probe SDK.
cc -std=c11 -Wall -Wextra -Werror \
  -I"$SDK/include" \
  "$DIR/native-probe/probe.c" \
  -L"$SDK/lib" -lsearchkernel_vespa \
  -Wl,-rpath,"$SDK/lib" \
  -o "$TMP/probe"

for symbol in \
  sk_grouping_plan_create \
  sk_grouping_expression_attribute \
  sk_grouping_expression_destroy \
  sk_grouping_aggregation_count \
  sk_grouping_aggregation_average \
  sk_grouping_aggregation_destroy \
  sk_grouping_plan_add_level \
  sk_grouping_level_add_count \
  sk_grouping_level_add_aggregation \
  sk_grouping_level_add_aggregation_handle \
  sk_engine_group_tree \
  sk_grouping_tree_result_root \
  sk_grouping_tree_node_group \
  sk_grouping_tree_node_key \
  sk_grouping_result_value_kind \
  sk_grouping_result_value_string \
  sk_grouping_result_value_integer \
  sk_grouping_result_value_float \
  sk_grouping_tree_node_output \
  sk_grouping_tree_output_kind \
  sk_grouping_tree_output_value; do
  nm -D "$TMP/probe" | grep "$symbol" >/dev/null || \
    fail_verification "C probe does not reference required symbol: $symbol"
done

# Reuse M9's already-frozen low-level schema/config fixture. Run both the
# canonical typed-handle construction and the permanent convenience wrappers
# through the same recursive scenario. Each output is checked against the same
# frozen expected JSON and direct Vespa oracle, then compared directly to its
# sibling path.
for mode in typed convenience; do
  DATA="$TMP/data-$mode"
  mkdir -p "$DATA"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$TMP/probe" "$DATA" "$ROOT/reference/vespa/m9/fixture" "$mode" \
    | "$DIR/canonicalize.py" > "$TMP/native-$mode.json"
  python3 "$ROOT/reference/vespa/common/compare_json.py" \
    "$EXPECTED" "$TMP/native-$mode.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    python3 "$ROOT/reference/vespa/common/compare_json.py" \
      "$TMP/oracle.json" "$TMP/native-$mode.json"
  fi
done
python3 "$ROOT/reference/vespa/common/compare_json.py" \
  "$TMP/native-typed.json" "$TMP/native-convenience.json"

echo "recursive grouping ABI verifier PASS"
