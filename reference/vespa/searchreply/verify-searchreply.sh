#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa

"$SCRIPT_DIR/total-hit-accuracy/verify-total-hit-accuracy.sh"

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

"$SCRIPT_DIR/oracle/build-cpp-oracle.sh"

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
  "$SCRIPT_DIR/oracle/run-cpp-oracle.sh" "$SCRIPT_DIR/scenario.json" \
  "$TMP/config" "$TMP/oracle-data" > "$TMP/oracle.json"
python3 "$ROOT/reference/vespa/common/compare_json.py" \
  "$SCRIPT_DIR/expected/searchreply.json" "$TMP/oracle.json"
LD_LIBRARY_PATH="$SK_NATIVE_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$PROBE" "$TMP/data" "$TMP/config" > "$TMP/native.json"
python3 "$ROOT/reference/vespa/common/compare_json.py" \
  "$SCRIPT_DIR/expected/searchreply.json" "$TMP/native.json"
python3 "$ROOT/reference/vespa/common/compare_json.py" \
  "$TMP/oracle.json" "$TMP/native.json"

HARDENING_SDK="${SK_NATIVE_HARDENING_SDK:-$ROOT/target/vespa-verifier/searchreply/sdk/hardening}"
SK_NATIVE_SDK="$HARDENING_SDK" \
SK_NATIVE_BUILD_DIR="$ROOT/target/vespa-verifier/searchreply/native-build/hardening" \
SK_NATIVE_ENABLE_FAILPOINTS=1 \
SK_NATIVE_REGENERATE_BINDINGS=0 \
  "$ROOT/core/native/vespa/build-sdk.sh"
HARDENING_PROBE="$TMP/searchreply_metadata_hardening_probe"
"${CXX:-c++}" -std=gnu++23 -O0 -g -Wall -Wextra -Werror -pthread \
  -I"$HARDENING_SDK/include" "$SCRIPT_DIR/native-probe/searchreply_metadata_hardening_probe.cpp" \
  -L"$HARDENING_SDK/lib" -Wl,-rpath,"$HARDENING_SDK/lib" \
  -lsearchkernel_vespa -o "$HARDENING_PROBE"
require_native_driver "$HARDENING_PROBE" sk_test_search_reply_metadata
LD_LIBRARY_PATH="$HARDENING_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  "$HARDENING_PROBE"
echo 'SearchReply semantics verification passed'
