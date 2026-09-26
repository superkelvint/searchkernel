#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"

# CARGO_TARGET_DIR is verification-plan-local under ./dev. Keep all expensive
# sanitizer build products beneath it so repeated M11 consumers in the same
# plan reuse them, while a distinct plan naturally gets a fresh artifact root.
M11_PLAN_CARGO_TARGET="$CARGO_TARGET_DIR"
TARGET="${M11_SANITIZER_TARGET:-$M11_PLAN_CARGO_TARGET/m11/sanitizer}"
mkdir -p "$TARGET"
M11_BASE_RUSTFLAGS="${RUSTFLAGS:-}"

sanitizer_sdk_ready() {
  local lane="$1"
  local sdk="$2"
  local manifest="$sdk/manifest.json"

  [[ -f "$manifest" ]] || return 1
  [[ -f "$sdk/include/searchkernel_vespa.h" ]] || return 1
  [[ -f "$sdk/lib/libsearchkernel_vespa.so.2" ]] || return 1
  grep -Eq '"hardening_failpoints"[[:space:]]*:[[:space:]]*true' "$manifest" || return 1
  grep -Eq "\"sanitizer\"[[:space:]]*:[[:space:]]*\"$lane\"" "$manifest"
}

run_tsan() {
  local sdk="$1"
  local suppression="$2"
  shift 2
  local output="$TARGET/tsan/runtime/tsan-attempt.log"
  local attempt
  local status
  local preload
  local mapping
  # GCC TSan can lose its shadow-map race with ASLR before main() starts.
  # Retry that startup-only failure, while preserving every real TSan report.
  for ((attempt = 1; attempt <= 100; attempt++)); do
    if TSAN_OPTIONS="suppressions=$suppression halt_on_error=1" \
      LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      "$@" >"$output" 2>&1; then
      status=0
      preload="${LD_PRELOAD:-}"
      unset LD_PRELOAD
      cat "$output"
      export LD_PRELOAD="$preload"
      return 0
    else
      status=$?
    fi
    preload="${LD_PRELOAD:-}"
    unset LD_PRELOAD
    if rg -q 'FATAL: ThreadSanitizer: unexpected memory mapping' "$output"; then
      mapping=1
    elif [[ "$status" == 139 ]] &&
      ! rg -q 'WARNING: ThreadSanitizer|SUMMARY: ThreadSanitizer|m11 failure' "$output"; then
      mapping=1
    else
      mapping=0
    fi
    export LD_PRELOAD="$preload"
    if [[ "$mapping" == 0 ]]; then
      preload="${LD_PRELOAD:-}"
      unset LD_PRELOAD
      cat "$output"
      export LD_PRELOAD="$preload"
      return "$status"
    fi
  done
  preload="${LD_PRELOAD:-}"
  unset LD_PRELOAD
  cat "$output"
  export LD_PRELOAD="$preload"
  return "$status"
}

run_lane() {
  local lane="$1"
  local lane_root="$TARGET/$lane"
  local sdk="$lane_root/sdk"
  local build="$lane_root/native-build"
  local cargo_target="$lane_root/cargo"
  local runtime="$lane_root/runtime"

  if ! sanitizer_sdk_ready "$lane" "$sdk"; then
    if using_published_vespa_toolchain; then
      M11_BASE_SDK="$(resolved_native_base_sdk)"
      SK_NATIVE_BASE_SDK="$M11_BASE_SDK" SK_NATIVE_ADAPTER_ONLY=1 VESPA_DEPS="$VESPA_DEPS" \
        SK_NATIVE_SDK="$sdk" SK_NATIVE_BUILD_DIR="$build" \
        "$ROOT/core/native/vespa/build-m11-sanitizer-sdk.sh" "$lane" "$sdk" "$build"
    else
      SK_NATIVE_SDK="$sdk" SK_NATIVE_BUILD_DIR="$build" \
        "$ROOT/core/native/vespa/build-m11-sanitizer-sdk.sh" "$lane" "$sdk" "$build"
    fi
  fi

  # Keep Cargo products lane-local. ASan/UBSan and TSan use different linker
  # flags, so sharing one target directory makes Cargo invalidate the other
  # lane on every pass even when the native SDK itself is reusable.
  export SK_NATIVE_SDK="$sdk"
  export CARGO_TARGET_DIR="$cargo_target"
  export RUSTFLAGS="$M11_BASE_RUSTFLAGS -C link-arg=-Wl,-rpath,$sdk/lib"
  cargo build --quiet --manifest-path "$M11/rust-driver/Cargo.toml"
  if [[ "$lane" == tsan ]]; then
    # Rust emits PIE executables by default. TSan's shadow-memory layout
    # cannot coexist with the randomized PIE mapping on this platform. Use
    # cargo rustc so -no-pie is applied only to the final binary, not to
    # proc-macro shared libraries built during cargo build.
    cargo rustc --quiet --manifest-path "$M11/rust-driver/Cargo.toml" \
      --bin searchkernel-m11-driver -- -C link-arg=-no-pie
  fi
  local binary="$CARGO_TARGET_DIR/debug/searchkernel-m11-driver"
  require_native_driver "$binary" sk_test_failpoint_set sk_test_failpoint_is_blocked

  # Scenario state is disposable even when build products are reusable.
  rm -rf "$runtime"
  mkdir -p "$runtime"

  local base_preload="${LD_PRELOAD:-}"
  local asan_runtime="$(cc -print-file-name=libasan.so)"
  local tsan_runtime="$(cc -print-file-name=libtsan.so.2)"
  [[ -f "$asan_runtime" ]] || fail_verification "libasan runtime was not found"
  [[ -f "$tsan_runtime" ]] || fail_verification "libtsan runtime was not found"
  local config="$runtime/config"
  local options=""
  if [[ "$lane" == asan-ubsan ]]; then
    options="$(tr '\n' ':' < "$M11/sanitizer/asan.options")"
  fi
  local ubsan_options="$(tr '\n' ':' < "$M11/sanitizer/ubsan.options")"
  local lsan="$M11/sanitizer/lsan.supp"
  local tsan="$M11/sanitizer/tsan.supp"
  case "$lane" in
    asan-ubsan) export LD_PRELOAD="$asan_runtime${base_preload:+:$base_preload}" ;;
    tsan) export LD_PRELOAD="$tsan_runtime${base_preload:+:$base_preload}" ;;
  esac
  case "$lane" in
    asan-ubsan)
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" compile "$config"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" malformed "$config" "$runtime/data"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" failures "$config" "$runtime/failures"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" lifecycle "$config" "$runtime/lifecycle"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" reopen "$config" "$runtime/reopen"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" advanced "$config" "$runtime/advanced"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" adapter "$config" "$runtime/adapter"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" concurrency "$config" "$runtime/concurrency"
      ;;
    tsan)
      run_tsan "$sdk" "$tsan" "$binary" compile "$config"
      run_tsan "$sdk" "$tsan" "$binary" concurrency "$config" "$runtime/data"
      run_tsan "$sdk" "$tsan" "$binary" lifecycle "$config" "$runtime/lifecycle"
      run_tsan "$sdk" "$tsan" "$binary" reopen "$config" "$runtime/reopen"
      run_tsan "$sdk" "$tsan" "$binary" advanced "$config" "$runtime/advanced"
      run_tsan "$sdk" "$tsan" "$binary" adapter "$config" "$runtime/adapter"
      ;;
  esac
  export LD_PRELOAD="$base_preload"
  export CARGO_TARGET_DIR="$M11_PLAN_CARGO_TARGET"
  export RUSTFLAGS="$M11_BASE_RUSTFLAGS"
}

for lane in ${M11_SANITIZER_LANES:-asan-ubsan tsan}; do
  run_lane "$lane"
done
echo "M11 sanitizer verification passed"
