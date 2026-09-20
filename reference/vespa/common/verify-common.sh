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
  local source_header="$REPO_ROOT/core/native/vespa/searchkernel_vespa.h"
  local sdk_header="$SDK/include/searchkernel_vespa.h"
  local rust_abi="$REPO_ROOT/core/crates/searchkernel-vespa-bindings/src/abi.rs"
  local rust_bindings="$REPO_ROOT/core/crates/searchkernel-vespa-bindings/src/bindings.rs"
  expected="$(expected_vespa_revision)"
  [[ -f "$source_header" ]] || fail_verification "source native ABI header is missing: $source_header"
  [[ -f "$sdk_header" ]] || fail_verification "SDK native ABI header is missing: $sdk_header"
  [[ -f "$rust_abi" ]] || fail_verification "Rust native ABI source is missing: $rust_abi"
  [[ -f "$rust_bindings" ]] || fail_verification "generated Rust bindings are missing: $rust_bindings"
  cmp -s "$source_header" "$sdk_header" ||
    fail_verification "SDK native ABI header differs from source header"
  python3 - "$manifest" "$REFERENCE_ROOT/PINNED_VESPA.json" \
    "$source_header" "$sdk_header" "$REFERENCE_ROOT/NATIVE_ABI_CONTRACT.h" "$rust_abi" "$rust_bindings" "$expected" <<'PY'
import json
import re
import sys

manifest_path, pin_path, source_header_path, sdk_header_path, contract_path, rust_path, bindings_path, expected_commit = sys.argv[1:]

def read_json(path):
    with open(path, encoding="utf-8") as stream:
        return json.load(stream)

def read_header_abi(path):
    with open(path, encoding="utf-8") as stream:
        matches = re.findall(
            r"^#define SK_NATIVE_ABI_VERSION ([0-9]+)U$", stream.read(), re.MULTILINE
        )
    if len(matches) != 1:
        raise SystemExit(
            f"verification error: expected exactly one SK_NATIVE_ABI_VERSION in {path}, found {len(matches)}"
        )
    return int(matches[0])

def read_rust_abi(path):
    with open(path, encoding="utf-8") as stream:
        matches = re.findall(
            r"^pub const EXPECTED_ABI: u32 = ([0-9]+);$", stream.read(), re.MULTILINE
        )
    if len(matches) != 1:
        raise SystemExit(
            f"verification error: expected exactly one EXPECTED_ABI in {path}, found {len(matches)}"
        )
    return int(matches[0])

def read_bindings_abi(path):
    with open(path, encoding="utf-8") as stream:
        matches = re.findall(
            r"^pub const SK_NATIVE_ABI_VERSION: u32 = ([0-9]+);$", stream.read(), re.MULTILINE
        )
    if len(matches) != 1:
        raise SystemExit(
            f"verification error: expected exactly one SK_NATIVE_ABI_VERSION in {path}, found {len(matches)}"
        )
    return int(matches[0])

manifest = read_json(manifest_path)
pin = read_json(pin_path)
source_abi = read_header_abi(source_header_path)
sdk_abi = read_header_abi(sdk_header_path)
contract_abi = read_header_abi(contract_path)
rust_abi = read_rust_abi(rust_path)
bindings_abi = read_bindings_abi(bindings_path)

authorities = {
    "source header": source_abi,
    "SDK header": sdk_abi,
    "SDK manifest": manifest.get("native_abi_version"),
    "Rust EXPECTED_ABI": rust_abi,
    "generated Rust bindings": bindings_abi,
    "reference ABI contract": contract_abi,
    "pinned metadata": pin.get("native_abi_version"),
}
if any(value != source_abi for value in authorities.values()):
    details = ", ".join(f"{name}={value!r}" for name, value in authorities.items())
    raise SystemExit(f"verification error: native ABI authorities disagree: {details}")
if manifest.get("vespa_commit") != expected_commit:
    raise SystemExit(
        f"verification error: SDK manifest Vespa commit {manifest.get('vespa_commit')!r} "
        f"does not match pinned commit {expected_commit!r}"
    )
if pin.get("commit") != expected_commit:
    raise SystemExit(
        f"verification error: pinned metadata commit {pin.get('commit')!r} "
        f"does not match expected commit {expected_commit!r}"
    )
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
  "$REPO_ROOT/tools/check-vespa-bindings.sh"
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets --all-features
}
