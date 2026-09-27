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
M11_DRIVER="$M11_CARGO_TARGET/debug/searchkernel-m11-driver"
M11_DEPS="${VESPA_DEPS:-$ROOT/vespa-deps}"

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
  # Rust driver through the normal pinned-base resolver; at execution time the
  # hardening adapter wins via LD_LIBRARY_PATH/RUNPATH.
  unset SK_NATIVE_SDK SK_NATIVE_BASE_SDK
  export CARGO_TARGET_DIR="$M11_CARGO_TARGET"
  export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,$M11_SDK/lib"
  cargo build --quiet --manifest-path "$M11/rust-driver/Cargo.toml"
  [[ -x "$M11_DRIVER" ]] || fail_verification "M11 driver was not built: $M11_DRIVER"
  require_native_driver "$M11_DRIVER" \
    sk_engine_open sk_engine_close sk_engine_destroy sk_engine_search \
    sk_engine_search_generic sk_engine_group_generic \
    sk_generic_search_request_add_grouping \
    sk_generic_search_result_match_feature_value \
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
