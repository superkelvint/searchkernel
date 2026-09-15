#!/usr/bin/env bash

# Shared acceptance helpers. Every milestone verifier sources this file so the
# acceptance rules do not drift between milestones.

COMMON_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REFERENCE_ROOT="$(cd "$COMMON_DIR/.." && pwd)"
REPO_ROOT="$(cd "$REFERENCE_ROOT/../.." && pwd)"
PIN_FILE="$REFERENCE_ROOT/PINNED_VESPA.json"
SDK="${SK_NATIVE_SDK:-$REPO_ROOT/core/native/sdk/linux-x86_64}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/vespa-verifier/cargo}"
if [[ -d "$SDK" ]]; then
  SDK="$(cd "$SDK" && pwd -P)"
fi
export SK_NATIVE_SDK="$SDK"
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
  local vespa="$REPO_ROOT/upstream/vespa"
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
  local expected
  local expected_abi
  expected="$(expected_vespa_revision)"
  expected_abi="$(sed -n 's/^#define SK_NATIVE_ABI_VERSION \([0-9][0-9]*\)U$/\1/p' \
    "$REFERENCE_ROOT/NATIVE_ABI_CONTRACT.h")"
  [[ -n "$expected_abi" ]] || fail_verification "native ABI version is missing from the contract"
  python3 - "$manifest" "$expected" "$expected_abi" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    manifest = json.load(stream)
assert manifest.get("native_abi_version") == int(sys.argv[3]), manifest
assert manifest.get("vespa_commit") == sys.argv[2], manifest
PY
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
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets --all-features
}
