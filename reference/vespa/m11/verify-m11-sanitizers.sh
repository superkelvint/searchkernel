#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"
"$M11/check-immutable.sh"

TARGET="$ROOT/target/vespa-verifier/m11/sanitizer"
rm -rf "$TARGET"
mkdir -p "$TARGET"

run_lane() {
  local lane="$1"
  local sdk="$TARGET/$lane/sdk"
  local build="$TARGET/$lane/native-build"
  SK_NATIVE_SDK="$sdk" SK_NATIVE_BUILD_DIR="$build" \
    "$ROOT/core/native/vespa/build-m11-sanitizer-sdk.sh" "$lane" "$sdk" "$build"
  export SK_NATIVE_SDK="$sdk"
  export CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
  cargo build --quiet --manifest-path "$M11/rust-driver/Cargo.toml"
  local binary="$CARGO_TARGET_DIR/debug/searchkernel-m11-driver"
  require_native_driver "$binary" sk_test_failpoint_set sk_test_failpoint_is_blocked
  local config="$TARGET/$lane/config"
  rm -rf "$config"
  LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$binary" compile "$config"
  local options="$(tr '\n' ':' < "$M11/sanitizer/${lane%%-*}.options" 2>/dev/null || true)"
  local ubsan_options="$(tr '\n' ':' < "$M11/sanitizer/ubsan.options")"
  local lsan="$M11/sanitizer/lsan.supp"
  local tsan="$M11/sanitizer/tsan.supp"
  case "$lane" in
    asan-ubsan)
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" malformed "$config" "$TARGET/$lane/data"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" failures "$config" "$TARGET/$lane/failures"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" lifecycle "$config" "$TARGET/$lane/lifecycle"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" reopen "$config" "$TARGET/$lane/reopen"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" concurrency "$config" "$TARGET/$lane/concurrency"
      ;;
    tsan)
      TSAN_OPTIONS="suppressions=$tsan halt_on_error=1" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" concurrency "$config" "$TARGET/$lane/data"
      TSAN_OPTIONS="suppressions=$tsan halt_on_error=1" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" lifecycle "$config" "$TARGET/$lane/lifecycle"
      TSAN_OPTIONS="suppressions=$tsan halt_on_error=1" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" reopen "$config" "$TARGET/$lane/reopen"
      ;;
  esac
}

run_lane asan-ubsan
run_lane tsan
echo "M11 sanitizer verification passed"
