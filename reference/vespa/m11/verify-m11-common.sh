#!/usr/bin/env bash
set -euo pipefail

M11="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M11/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"

M11_SDK="${M11_SDK:-$ROOT/target/vespa-verifier/m11/sdk/hardening}"
M11_BUILD="${M11_BUILD:-$ROOT/target/vespa-verifier/m11/native-build/hardening}"
M11_SDK_CONFIG="$M11_SDK/share/embedded_crud_cfg"
M11_CONFIG="${M11_CONFIG:-$ROOT/target/vespa-verifier/m11/config}"
M11_CARGO_TARGET="$CARGO_TARGET_DIR"
M11_DRIVER="$M11_CARGO_TARGET/debug/examples/searchkernel-m11-driver"
M11_DEPS="${VESPA_DEPS:-$ROOT/vespa-deps}"

m11_adapter_path_from_ldd() {
  awk '/libsearchkernel_vespa\.so/ {
    for (i = 1; i <= NF; ++i) {
      if ($i ~ /^\//) { print $i; exit }
    }
  }'
}

stage_m11_adapter_for_executable_dir() {
  local executable_dir="${1:?stage_m11_adapter_for_executable_dir <dir> <runtime-sdk>}"
  local runtime_sdk="${2:?stage_m11_adapter_for_executable_dir <dir> <runtime-sdk>}"
  local staged_dir="$executable_dir/.searchkernel-native"
  local staged="$staged_dir/libsearchkernel_vespa.so.2"
  local runtime="$runtime_sdk/lib/libsearchkernel_vespa.so.2"
  [[ -f "$runtime" ]] || fail_verification "missing M11 runtime adapter: $runtime"
  mkdir -p "$staged_dir"
  rm -f "$staged"
  cp "$runtime" "$staged"
}

stage_m11_driver_adapter() {
  local driver="${1:?stage_m11_driver_adapter <driver> <runtime-sdk>}"
  local runtime_sdk="${2:?stage_m11_driver_adapter <driver> <runtime-sdk>}"
  stage_m11_adapter_for_executable_dir "$(dirname "$driver")" "$runtime_sdk"
}

prepare_hardening_sdk() {
  if [[ -f "$M11_SDK/manifest.json" ]]; then
    python3 - "$M11_SDK/manifest.json" <<'PY'
import json
import sys
manifest = json.load(open(sys.argv[1], encoding="utf-8"))
if not manifest.get("hardening_failpoints", False):
    raise SystemExit("SDK is not a hardening/failpoint SDK")
PY
  else
    mkdir -p "$(dirname "$M11_SDK")" "$M11_BUILD"
    if using_published_vespa_toolchain; then
      M11_BASE_SDK="$(resolved_native_base_sdk)"
      SK_NATIVE_SDK="$M11_SDK" \
      SK_NATIVE_BUILD_DIR="$M11_BUILD" \
      SK_NATIVE_BASE_SDK="$M11_BASE_SDK" \
      SK_NATIVE_ADAPTER_ONLY=1 \
      VESPA_DEPS="$M11_DEPS" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_ENABLE_FAILPOINTS=1 \
        "$ROOT/core/native/vespa/build-sdk.sh"
    else
      SK_NATIVE_SDK="$M11_SDK" \
      SK_NATIVE_BUILD_DIR="$M11_BUILD" \
      VESPA_DEPS="$M11_DEPS" \
        SK_NATIVE_REGENERATE_BINDINGS=0 \
        SK_NATIVE_ENABLE_FAILPOINTS=1 \
        "$ROOT/core/native/vespa/build-sdk.sh"
    fi
  fi
  [[ -d "$M11_SDK_CONFIG" ]] || fail_verification "missing M11 SDK config: $M11_SDK_CONFIG"
  require_sdk_manifest "$M11_SDK/manifest.json"
}

build_m11_driver() {
  # M11_SDK is an adapter-only hardening runtime override, not a full SDK
  # package accepted by Cargo's explicit-SDK provenance contract. Build the
  # Rust driver through the normal pinned-base resolver; after Cargo stages its
  # self-resolving adapter, repoint that staged path to the hardening runtime.
  unset SK_NATIVE_SDK SK_NATIVE_BASE_SDK
  export CARGO_TARGET_DIR="$M11_CARGO_TARGET"
  export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,$M11_SDK/lib"
  # Exact-head verification must never execute a driver binary left behind by
  # an earlier revision in the persistent shared Cargo target directory.
  rm -f "$M11_DRIVER"
  cargo build --quiet -p searchkernel --features native,internal-api,m11-verifier-hardening --example searchkernel-m11-driver
  [[ -x "$M11_DRIVER" ]] || fail_verification "M11 driver was not built: $M11_DRIVER"
  # Native consumer binaries intentionally carry a slash-bearing
  # $ORIGIN/.searchkernel-native DT_NEEDED entry. LD_LIBRARY_PATH cannot
  # override such an entry, so repoint the staged adapter explicitly for M11.
  stage_m11_driver_adapter "$M11_DRIVER" "$M11_SDK"
  require_native_driver "$M11_DRIVER" \
    sk_engine_open sk_engine_close sk_engine_destroy \
    sk_engine_search_generic sk_engine_group_tree \
    sk_generic_search_request_add_grouping \
    sk_generic_search_result_match_feature_value_ref \
    sk_test_failpoint_set sk_test_failpoint_is_blocked
  rm -rf "$M11_CONFIG"
  mkdir -p "$(dirname "$M11_CONFIG")"
  LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$M11_DRIVER" compile "$M11_CONFIG"
}

run_m11_driver() {
  local mode="$1"
  local data="$2"
  mkdir -p "$data"
  LD_LIBRARY_PATH="$M11_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$M11_DRIVER" "$mode" "$M11_CONFIG" "$data"
}
