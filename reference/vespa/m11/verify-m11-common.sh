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
    SK_NATIVE_SDK="$M11_SDK" \
    SK_NATIVE_BUILD_DIR="$M11_BUILD" \
    SK_NATIVE_ENABLE_FAILPOINTS=1 \
      "$ROOT/native/vespa/build-sdk.sh"
  fi
  [[ -d "$M11_SDK_CONFIG" ]] || fail_verification "missing M11 SDK config: $M11_SDK_CONFIG"
  require_sdk_manifest "$M11_SDK/manifest.json"
}

build_m11_driver() {
  export SK_NATIVE_SDK="$M11_SDK"
  export CARGO_TARGET_DIR="$M11_CARGO_TARGET"
  cargo build --quiet --manifest-path "$M11/rust-driver/Cargo.toml"
  [[ -x "$M11_DRIVER" ]] || fail_verification "M11 driver was not built: $M11_DRIVER"
  require_native_driver "$M11_DRIVER" \
    sk_engine_open sk_engine_close sk_engine_destroy sk_engine_search \
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
