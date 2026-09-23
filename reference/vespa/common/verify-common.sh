#!/usr/bin/env bash

# Shared acceptance helpers. Every milestone verifier sources this file so the
# acceptance rules do not drift between milestones.

COMMON_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REFERENCE_ROOT="$(cd "$COMMON_DIR/.." && pwd)"
REPO_ROOT="$(cd "$REFERENCE_ROOT/../.." && pwd)"
PIN_FILE="$REFERENCE_ROOT/PINNED_VESPA.json"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/vespa-verifier/cargo}"
if [[ -n "${SK_NATIVE_SDK:-}" ]]; then
  SDK="$SK_NATIVE_SDK"
  if [[ -d "$SDK" ]]; then
    SDK="$(cd "$SDK" && pwd -P)"
  fi
  export SK_NATIVE_SDK="$SDK"
else
  # Native verifier scripts need a Search Kernel adapter/header/manifest view,
  # but the pinned Vespa/runtime closure is a machine-global prerequisite.
  # Build only the small adapter locally and leave Cargo on the shared-base path.
  unset SK_NATIVE_SDK
  BASE_SDK=$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" path)
  VESPA_ROOT=$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" vespa-root)
  SDK="$CARGO_TARGET_DIR/native-verifier-sdk"
  SDK_BUILD="$CARGO_TARGET_DIR/native-verifier-sdk-build"
  SK_NATIVE_SDK="$SDK" \
  SK_NATIVE_BUILD_DIR="$SDK_BUILD" \
  SK_NATIVE_BASE_SDK="$BASE_SDK" \
  SK_NATIVE_ADAPTER_ONLY=1 \
  SK_NATIVE_REGENERATE_BINDINGS=0 \
  SK_NATIVE_SKIP_BINDINGS_CHECK=1 \
  VESPA_ROOT="$VESPA_ROOT" \
    bash "$REPO_ROOT/core/native/vespa/build-sdk.sh" >/dev/null
fi
if [[ -z "${VESPA_ROOT:-}" ]]; then
  VESPA_ROOT=$(bash "$REPO_ROOT/tools/resolve-native-sdk.sh" vespa-root)
fi
export VESPA_ROOT
export CARGO_TARGET_DIR

expected_vespa_revision() {
  python3 - "$PIN_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    pin = json.load(stream)
print(pin["commit"])
PY
}

fail_verification() {
  echo "verification error: $*" >&2
  exit 1
}

require_pinned_vespa() {
  local vespa="$VESPA_ROOT"
  local expected actual
  expected="$(expected_vespa_revision)"
  [[ -d "$vespa/.git" || -f "$vespa/.git" ]] ||
    fail_verification "missing pinned Vespa checkout: $vespa"
  # The verifier is commonly run as root inside the Vespa build container
  # while the checkout is owned by the image's unprivileged build user. Tell
  # Git exactly which checkout is being inspected; this does not relax the
  # pinned-revision or clean-tree checks below.
  actual="$(git -c safe.directory="$vespa" -C "$vespa" rev-parse HEAD 2>/dev/null)" ||
    fail_verification "cannot determine Vespa checkout revision: $vespa"
  [[ "$actual" == "$expected" ]] ||
    fail_verification "expected Vespa $expected, found $actual"
  git -c safe.directory="$vespa" -C "$vespa" diff --quiet -- ||
    fail_verification "tracked Vespa source is dirty: $vespa"
  git -c safe.directory="$vespa" -C "$vespa" diff --cached --quiet -- ||
    fail_verification "staged Vespa source changes found: $vespa"
}

require_sdk_manifest() {
  local manifest="$1"
  local source_header="$REPO_ROOT/core/native/vespa/searchkernel_vespa.h"
  local sdk_header="$SDK/include/searchkernel_vespa.h"

  [[ -f "$manifest" ]] || fail_verification "SDK manifest is missing: $manifest"
  [[ -f "$source_header" ]] || fail_verification "source native ABI header is missing: $source_header"
  [[ -f "$sdk_header" ]] || fail_verification "SDK native ABI header is missing: $sdk_header"
  if find "$SDK/lib" -type f \( -name '*.h' -o -name '*.hh' -o -name '*.hpp' \) -print -quit 2>/dev/null | grep -q .; then
    fail_verification "SDK lib directory contains native headers; rebuild the SDK from a clean staging directory"
  fi
  cmp -s "$source_header" "$sdk_header" ||
    fail_verification "SDK native ABI header differs from source header"

  if ! python3 "$REPO_ROOT/tools/check-native-abi-authorities.py" \
      --root "$REPO_ROOT" \
      --sdk-header "$sdk_header" \
      --sdk-manifest "$manifest"; then
    fail_verification "native ABI authority agreement failed"
  fi
}

check_fixture_literals() {
  python3 "$COMMON_DIR/check-fixture-literals.py" "$@"
}

require_native_driver() {
  local binary="$1"
  shift
  [[ -x "$binary" ]] || fail_verification "driver binary is missing or not executable: $binary"
  # Read the complete ldd output. With pipefail enabled, grep -q can make ldd
  # exit on SIGPIPE after finding the library and falsely report failure.
  if ldd "$binary" | grep 'libsearchkernel_vespa' >/dev/null; then
    :
  else
    echo "native driver is not dynamically linked to libsearchkernel_vespa: $binary" >&2
    ldd "$binary" >&2 || true
    exit 1
  fi
  local symbol
  for symbol in "$@"; do
    nm -D "$binary" 2>/dev/null | grep "$symbol" >/dev/null ||
      fail_verification "native driver does not reference required symbol: $symbol"
  done
}

run_workspace_quality() {
  "$REPO_ROOT/tools/check-vespa-bindings.sh"
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets --all-features
}
