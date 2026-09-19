#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M10="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M10/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
export LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
SCENARIO="$SCRIPT_DIR/scenario.json"
EXPECTED="$SCRIPT_DIR/expected.json"
[[ -f "$EXPECTED" ]] || fail_verification "feature-value golden is missing: $EXPECTED"

# The direct oracle is the authority for the opaque bytes. Rebuild it from the
# current pinned checkout because this verifier extends its tensor fixture.
"$M10/oracle/build-cpp-oracle.sh"
cargo build --quiet --manifest-path "$M10/rust-driver/Cargo.toml" --bin searchkernel-m10-driver

TARGET="$REPO_ROOT/target/vespa-verifier/m10-feature-values"
mkdir -p "$TARGET"
cc -std=c11 -Wall -Wextra -Werror \
  -I"$SDK/include" \
  "$SCRIPT_DIR/native-probe/probe.c" \
  -L"$SDK/lib" -lsearchkernel_vespa \
  -Wl,-rpath,"$SDK/lib" \
  -o "$TARGET/probe"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

"$M10/oracle/run-cpp-oracle.sh" "$SCENARIO" |
  "$M10/canonicalize.py" > "$TMP/oracle.json"
"$M10/compare.py" "$EXPECTED" "$TMP/oracle.json"

CFG="$TMP/config"
DATA="$TMP/data"
mkdir -p "$DATA"
cargo run --quiet --manifest-path "$M10/rust-driver/Cargo.toml" \
  --bin searchkernel-m10-driver -- "$SCENARIO" "$CFG" "$DATA" |
  "$M10/canonicalize.py" > "$TMP/rust.json"
"$M10/compare.py" "$EXPECTED" "$TMP/rust.json"
"$M10/compare.py" "$TMP/oracle.json" "$TMP/rust.json"

"$TARGET/probe" "$DATA" "$CFG" |
  "$M10/canonicalize.py" > "$TMP/native.json"
"$M10/compare.py" "$EXPECTED" "$TMP/native.json"

echo 'M10 feature-value verification passed'
