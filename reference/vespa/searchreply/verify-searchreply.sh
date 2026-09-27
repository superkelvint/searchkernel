#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi

"$SCRIPT_DIR/total-hit-accuracy/verify-total-hit-accuracy.sh"

grep -q 'MATCH_PHASE = 0x01' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'TIMEOUT     = 0x02' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'ANN_TIMEOUT = 0x04' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'my_issues' "$SCRIPT_DIR/SOURCE_TRACE.md"
grep -q 'maxfiltercoverage=0.9' "$SCRIPT_DIR/CONTRACT.md"
grep -q 'wasDegradedByMatchPhase' "$ROOT/core/native/vespa/search.cc"
grep -q 'SK_DEGRADATION_MATCH_PHASE' "$ROOT/core/native/vespa/search.cc"
grep -q 'wasDegradedByTimeout' "$ROOT/core/native/vespa/search.cc"
grep -q 'SK_DEGRADATION_TIMEOUT' "$ROOT/core/native/vespa/search.cc"
grep -q 'was_degraded_by_ann_timeout' "$ROOT/core/native/vespa/search.cc"
grep -q 'SK_DEGRADATION_ANN_TIMEOUT' "$ROOT/core/native/vespa/search.cc"

cargo fmt --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" -- --check
cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml"
CONFIG_BIN="$CARGO_TARGET_DIR/debug/searchkernel-searchreply-config"
[[ -x "$CONFIG_BIN" ]] || fail_verification "SearchReply config compiler was not built: $CONFIG_BIN"

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  "$SCRIPT_DIR/oracle/build-cpp-oracle.sh"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
run_native_executable "$CONFIG_BIN" "$TMP/config"

PROBE="$TMP/searchreply_probe"
"${CXX:-c++}" -std=gnu++23 -O0 -g -Wall -Wextra -Werror -pthread \
  -I"$SDK/include" "$SCRIPT_DIR/native-probe/searchreply_probe.cpp" \
  -L"$SDK/lib" -Wl,-rpath,"$SDK/lib" \
  -lsearchkernel_vespa -o "$PROBE"
require_native_driver "$PROBE" \
  sk_engine_search_generic \
  sk_generic_search_result_coverage_active \
  sk_generic_search_result_degradation_flags \
  sk_generic_search_result_issue_message

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$SCRIPT_DIR/oracle/run-cpp-oracle.sh" "$SCRIPT_DIR/scenario.json" \
    "$TMP/config" "$TMP/oracle-data" > "$TMP/oracle.json"
  python3 "$ROOT/reference/vespa/common/compare_json.py" \
    "$SCRIPT_DIR/expected/searchreply.json" "$TMP/oracle.json"
fi
run_native_executable "$PROBE" "$TMP/data" "$TMP/config" > "$TMP/native.json"
python3 "$ROOT/reference/vespa/common/compare_json.py" \
  "$SCRIPT_DIR/expected/searchreply.json" "$TMP/native.json"
if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  python3 "$ROOT/reference/vespa/common/compare_json.py" \
    "$TMP/oracle.json" "$TMP/native.json"
fi

HARDENING_SDK="${SK_NATIVE_HARDENING_SDK:-$ROOT/target/vespa-verifier/searchreply/sdk/hardening}"
HARDENING_RUNTIME_LIB_DIRS=
if using_published_vespa_toolchain; then
  SEARCHREPLY_BASE_SDK="$(resolved_native_base_sdk)"
  HARDENING_RUNTIME_LIB_DIRS="$HARDENING_SDK/lib:$SEARCHREPLY_BASE_SDK/lib"
  SK_NATIVE_SDK="$HARDENING_SDK" \
  SK_NATIVE_BUILD_DIR="$ROOT/target/vespa-verifier/searchreply/native-build/hardening" \
  SK_NATIVE_BASE_SDK="$SEARCHREPLY_BASE_SDK" \
  SK_NATIVE_ADAPTER_ONLY=1 \
  SK_NATIVE_ENABLE_FAILPOINTS=1 \
  SK_NATIVE_REGENERATE_BINDINGS=0 \
    "$ROOT/core/native/vespa/build-sdk.sh"
else
  HARDENING_RUNTIME_LIB_DIRS="$HARDENING_SDK/lib"
  SK_NATIVE_SDK="$HARDENING_SDK" \
  SK_NATIVE_BUILD_DIR="$ROOT/target/vespa-verifier/searchreply/native-build/hardening" \
  SK_NATIVE_ENABLE_FAILPOINTS=1 \
  SK_NATIVE_REGENERATE_BINDINGS=0 \
    "$ROOT/core/native/vespa/build-sdk.sh"
fi
HARDENING_PROBE="$TMP/searchreply_metadata_hardening_probe"
"${CXX:-c++}" -std=gnu++23 -O0 -g -Wall -Wextra -Werror -pthread \
  -I"$HARDENING_SDK/include" "$SCRIPT_DIR/native-probe/searchreply_metadata_hardening_probe.cpp" \
  -L"$HARDENING_SDK/lib" -Wl,-rpath,"$HARDENING_SDK/lib" \
  -lsearchkernel_vespa -o "$HARDENING_PROBE"
require_native_driver "$HARDENING_PROBE" sk_test_search_reply_metadata
(
  export SEARCHKERNEL_RUNTIME_LIB_DIRS="$HARDENING_RUNTIME_LIB_DIRS"
  run_native_executable "$HARDENING_PROBE"
)
echo 'SearchReply semantics verification passed'
