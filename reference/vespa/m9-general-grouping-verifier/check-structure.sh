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
if grep -R --line-number --include='*.cpp' --include='*.h' 'searchkernel-ir\|GroupingRequestIr' \
    "$ROOT/core/native/vespa" >/dev/null; then
  echo "native grouping implementation depends on C-series IR terminology" >&2
  exit 1
fi

# The general grouping path is spread across the engine lowering/execution
# helpers and the recursive result decoders. Extract those concrete bodies and
# inspect only executable source: comments and string literals are masked before
# matching so documentation cannot satisfy this anti-shortcut verifier.
NATIVE_CPP="${SK_GROUPING_NATIVE_CPP:-$ROOT/core/native/vespa/searchkernel_vespa.cpp}"
python3 - "$NATIVE_CPP" <<'PY'
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")


def mask_comments_and_literals(text):
    """Replace non-executable C++ text while preserving source positions."""
    result = list(text)
    index = 0
    state = "code"
    while index < len(text):
        if state == "code":
            if text.startswith("//", index):
                result[index:index + 2] = "  "
                index += 2
                state = "line-comment"
            elif text.startswith("/*", index):
                result[index:index + 2] = "  "
                index += 2
                state = "block-comment"
            elif text[index] in ('"', "'"):
                quote = text[index]
                result[index] = " "
                index += 1
                state = ("string", quote)
            else:
                index += 1
        elif state == "line-comment":
            if text[index] == "\n":
                state = "code"
            else:
                result[index] = " "
            index += 1
        elif state == "block-comment":
            if text.startswith("*/", index):
                result[index:index + 2] = "  "
                index += 2
                state = "code"
            else:
                if text[index] != "\n":
                    result[index] = " "
                index += 1
        else:
            quote = state[1]
            if text[index] == "\\":
                result[index] = " "
                if index + 1 < len(text):
                    if text[index + 1] != "\n":
                        result[index + 1] = " "
                    index += 2
                else:
                    index += 1
            elif text[index] == quote:
                result[index] = " "
                index += 1
                state = "code"
            else:
                if text[index] != "\n":
                    result[index] = " "
                index += 1
    return "".join(result)


masked = mask_comments_and_literals(source)


def function_body(name, signature):
    matches = list(re.finditer(signature, masked, re.MULTILINE))
    if not matches:
        raise SystemExit(f"missing executable grouping function: {name}")
    opening = -1
    for match in matches:
        candidate = match.end()
        while candidate < len(masked) and masked[candidate].isspace():
            candidate += 1
        if candidate < len(masked) and masked[candidate] == "{":
            opening = candidate
            break
    if opening < 0:
        raise SystemExit(f"missing body for grouping function: {name}")
    depth = 0
    for index in range(opening, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return masked[opening + 1:index]
    raise SystemExit(f"unterminated body for grouping function: {name}")


def require(body, pattern, description):
    if not re.search(pattern, body, re.MULTILINE | re.DOTALL):
        raise SystemExit(f"{description} is missing from executable grouping code")


make_grouping = function_body(
    "EmbeddedEngine::make_grouping(plan)",
    r"\bGrouping\s+make_grouping\s*\(\s*const\s+sk_grouping_plan\s*&\s*plan\s*\)\s*const\s*",
)
group_tree = function_body(
    "EmbeddedEngine::group_tree",
    r"\bsk_grouping_tree_result\s+group_tree\s*\(\s*const\s+NativeQueryState\s*&\s*query\s*,\s*const\s+sk_grouping_plan\s*&\s*plan\s*\)\s*const\s*",
)
tree_node = function_body(
    "grouping_tree_node",
    r"\bstd::unique_ptr\s*<\s*sk_grouping_tree_node\s*>\s+grouping_tree_node\s*\(\s*const\s+Group\s*&\s*group\s*,\s*bool\s+include_key\s*\)",
)
tree_output = function_body(
    "grouping_tree_output",
    r"\bstd::unique_ptr\s*<\s*sk_grouping_tree_output\s*>\s+grouping_tree_output\s*\(\s*const\s+AggregationResult\s*&\s*aggregation\s*\)",
)
result_value = function_body(
    "grouping_result_value",
    r"\bstd::unique_ptr\s*<\s*sk_grouping_result_value\s*>\s+grouping_result_value\s*\(\s*const\s+search::expression::ResultNode\s*&\s*node\s*\)",
)

# Request lowering must construct a real, ordered Vespa grouping plan. The
# expression clones are important: fixture field names must not be substituted
# for the caller-provided classifier and aggregation expressions.
require(make_grouping, r"\bGrouping\s+grouping\s*;", "Grouping construction")
require(make_grouping, r"\bGroupingLevel\s*\(", "GroupingLevel construction")
require(make_grouping, r"auto\s+level\s*=\s*GroupingLevel\s*\(\)\.setExpression\s*\(", "level classifier lowering")
require(make_grouping, r"dynamic_cast\s*<\s*const\s+AttributeNode\s*\*", "attribute classifier/expression lowering")
require(make_grouping, r"std::make_unique\s*<\s*CountAggregationResult\s*>", "count aggregation lowering")
require(make_grouping, r"std::make_unique\s*<\s*AverageAggregationResult\s*>", "average aggregation lowering")
require(make_grouping, r"count\s*->\s*setExpression\s*\(", "count classifier expression lowering")
require(make_grouping, r"average\s*->\s*setExpression\s*\(", "average aggregation expression lowering")
require(make_grouping, r"level_state\s*->\s*classifier\s*->\s*clone\s*\(", "classifier expression lowering")
require(make_grouping, r"aggregation_state\.expression\s*->\s*clone\s*\(", "aggregation expression lowering")
require(make_grouping, r"for\s*\([^)]*level_state[^)]*plan\.levels", "ordered plan-level iteration")
require(make_grouping, r"grouping\.addLevel\s*\(", "plan level attachment")
for fixture_name in ("category", "qty", "price", "A", "B"):
    if re.search(rf"\b{re.escape(fixture_name)}\b", make_grouping):
        raise SystemExit(
            f"general grouping lowering special-cases fixture name {fixture_name}"
        )

# Execution must go through SearchRequest.groupSpec and the embedded matcher,
# then decode the real SearchReply grouping payload.
require(group_tree, r"\bSearchRequest\s+request\s*;", "SearchRequest construction")
require(group_tree, r"\bmake_grouping\s*\(\s*plan\s*\)", "grouping plan lowering call")
require(group_tree, r"grouping\.serialize\s*\(", "Grouping serialization")
require(group_tree, r"request\.groupSpec", "SearchRequest.groupSpec assignment")
if len(re.findall(r"\b_db\s*->\s*match\s*\(", group_tree)) != 1:
    raise SystemExit("group_tree must call the embedded Vespa matcher exactly once")
require(group_tree, r"reply\s*->\s*groupResult", "SearchReply.groupResult consumption")
require(group_tree, r"\bGrouping\s+result_grouping\s*;", "result Grouping construction")
require(group_tree, r"result_grouping\.deserialize\s*\(", "Grouping result deserialization")
require(group_tree, r"grouping_tree_node\s*\(\s*result_grouping\.getRoot\s*\(\)", "recursive root decoding")
for shortcut in (r"\bsk_engine_search\s*\(", r"\bsk_engine_search_generic\s*\(",
                 r"\bsearch_query\s*\(", r"\bgroup_local\s*\("):
    if re.search(shortcut, group_tree):
        raise SystemExit(f"group_tree uses prohibited shortcut {shortcut}")

# Result decoding must consume Vespa's actual Group and AggregationResult
# objects and recurse through their children, rather than issuing searches or
# rebuilding the fixture in the wrapper.
require(tree_node, r"group\.getId\s*\(\)", "Vespa group ID decoding")
require(tree_node, r"group\.getAggregationResult\s*\(", "Vespa aggregation-result decoding")
require(tree_node, r"group\.getChildrenSize\s*\(\)", "recursive child-count decoding")
require(tree_node, r"group\.getChild\s*\(", "recursive child decoding")
require(tree_node, r"grouping_tree_node\s*\(\s*group\.getChild", "recursive grouping tree descent")
require(tree_output, r"aggregation\.getClass\s*\(\)\.inherits\s*\(", "aggregation subclass dispatch")
require(tree_output, r"CountAggregationResult::classId", "count result subclass dispatch")
require(tree_output, r"AverageAggregationResult::classId", "average result subclass dispatch")
require(tree_output, r"static_cast\s*<\s*const\s+CountAggregationResult\s*&", "count result decoding")
require(tree_output, r"static_cast\s*<\s*const\s+AverageAggregationResult\s*&", "average result decoding")
require(result_value, r"node\.getClass\s*\(\)\.inherits\s*\(", "result-node subclass dispatch")
for result_type in ("StringResultNode", "Int64ResultNode", "FloatResultNode"):
    require(result_value, rf"{result_type}::classId", f"{result_type} decoding")
PY

echo "recursive grouping ABI structure checks passed"
