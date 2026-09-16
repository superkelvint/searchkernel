#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
python3 "$M11/validate.py"

TARGET="$ROOT/target/vespa-verifier/m11/sanitizer"
rm -rf "$TARGET"
mkdir -p "$TARGET"
M11_BASE_RUSTFLAGS="${RUSTFLAGS:-}"

run_tsan() {
  local sdk="$1"
  local suppression="$2"
  shift 2
  local output="$TARGET/tsan-attempt.log"
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
  local sdk="$TARGET/$lane/sdk"
  local build="$TARGET/$lane/native-build"
  SK_NATIVE_SDK="$sdk" SK_NATIVE_BUILD_DIR="$build" \
    "$ROOT/core/native/vespa/build-m11-sanitizer-sdk.sh" "$lane" "$sdk" "$build"
  export SK_NATIVE_SDK="$sdk"
  export CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
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
  local base_preload="${LD_PRELOAD:-}"
  local asan_runtime="$(cc -print-file-name=libasan.so)"
  local tsan_runtime="$(cc -print-file-name=libtsan.so.2)"
  [[ -f "$asan_runtime" ]] || fail_verification "libasan runtime was not found"
  [[ -f "$tsan_runtime" ]] || fail_verification "libtsan runtime was not found"
  case "$lane" in
    asan-ubsan) export LD_PRELOAD="$asan_runtime${base_preload:+:$base_preload}" ;;
    tsan) export LD_PRELOAD="$tsan_runtime${base_preload:+:$base_preload}" ;;
  esac
  local config="$TARGET/$lane/config"
  rm -rf "$config"
  local options="$(tr '\n' ':' < "$M11/sanitizer/${lane%%-*}.options" 2>/dev/null || true)"
  local ubsan_options="$(tr '\n' ':' < "$M11/sanitizer/ubsan.options")"
  local lsan="$M11/sanitizer/lsan.supp"
  local tsan="$M11/sanitizer/tsan.supp"
  case "$lane" in
    asan-ubsan)
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" compile "$config"
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
        "$binary" advanced "$config" "$TARGET/$lane/advanced"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" adapter "$config" "$TARGET/$lane/adapter"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" c5 "$config" "$TARGET/$lane/c5"
        ASAN_OPTIONS="$options" UBSAN_OPTIONS="$ubsan_options" LSAN_OPTIONS="suppressions=$lsan" \
        LD_LIBRARY_PATH="$sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
        "$binary" concurrency "$config" "$TARGET/$lane/concurrency"
      ;;
    tsan)
      run_tsan "$sdk" "$tsan" "$binary" compile "$config"
      run_tsan "$sdk" "$tsan" "$binary" concurrency "$config" "$TARGET/$lane/data"
      run_tsan "$sdk" "$tsan" "$binary" lifecycle "$config" "$TARGET/$lane/lifecycle"
      run_tsan "$sdk" "$tsan" "$binary" reopen "$config" "$TARGET/$lane/reopen"
      run_tsan "$sdk" "$tsan" "$binary" advanced "$config" "$TARGET/$lane/advanced"
      run_tsan "$sdk" "$tsan" "$binary" adapter "$config" "$TARGET/$lane/adapter"
      run_tsan "$sdk" "$tsan" "$binary" c5 "$config" "$TARGET/$lane/c5"
      ;;
  esac
  export LD_PRELOAD="$base_preload"
}

for lane in ${M11_SANITIZER_LANES:-asan-ubsan tsan}; do
  run_lane "$lane"
done
echo "M11 sanitizer verification passed"
