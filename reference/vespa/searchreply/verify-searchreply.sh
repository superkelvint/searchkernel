#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa

grep -q 'MATCH_PHASE = 0x01' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'TIMEOUT     = 0x02' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'ANN_TIMEOUT = 0x04' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'my_issues' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'maxfiltercoverage=0.9' "$SCRIPT_DIR/CONTRACT.md"
grep -q 'wasDegradedByMatchPhase' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"
grep -q 'SK_DEGRADATION_MATCH_PHASE' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"
grep -q 'wasDegradedByTimeout' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"
grep -q 'SK_DEGRADATION_TIMEOUT' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"
grep -q 'was_degraded_by_ann_timeout' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"
grep -q 'SK_DEGRADATION_ANN_TIMEOUT' "$ROOT/core/native/vespa/searchkernel_vespa.cpp"

cargo fmt --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- --check
cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml"
CONFIG_BIN="$CARGO_TARGET_DIR/debug/searchkernel-searchreply-config"
[[ -x "$CONFIG_BIN" ]] || fail_verification "SearchReply config compiler was not built: $CONFIG_BIN"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
"$CONFIG_BIN" "$TMP/config"

PROBE="$TMP/searchreply_probe"
"${CXX:-c++}" -std=gnu++23 -O0 -g -Wall -Wextra -Werror -pthread \
  -I"$SK_NATIVE_SDK/include" "$SCRIPT_DIR/native-probe/searchreply_probe.cpp" \
  -L"$SK_NATIVE_SDK/lib" -Wl,-rpath,"$SK_NATIVE_SDK/lib" \
  -lsearchkernel_vespa -o "$PROBE"
require_native_driver "$PROBE" \
  sk_engine_search_generic \
  sk_generic_search_result_coverage_active \
  sk_generic_search_result_degradation_flags \
  sk_generic_search_result_issue_message

LD_LIBRARY_PATH="$SK_NATIVE_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$PROBE" "$TMP/data" "$TMP/config"
echo 'SearchReply semantics verification passed'
