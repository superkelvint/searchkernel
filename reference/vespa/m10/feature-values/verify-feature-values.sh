#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M10="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M10/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi
export LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
SCENARIO="$SCRIPT_DIR/scenario.json"
EXPECTED="$SCRIPT_DIR/expected.json"
[[ -f "$EXPECTED" ]] || fail_verification "feature-value golden is missing: $EXPECTED"

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  "$M10/oracle/build-cpp-oracle.sh"
fi
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

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  "$M10/oracle/run-cpp-oracle.sh" "$SCENARIO" |
    "$M10/canonicalize.py" > "$TMP/oracle.json"
  "$M10/compare.py" "$EXPECTED" "$TMP/oracle.json"
fi

CFG="$TMP/config"
DATA="$TMP/data"
mkdir -p "$DATA"
cargo run --quiet --manifest-path "$M10/rust-driver/Cargo.toml" \
  --bin searchkernel-m10-driver -- "$SCENARIO" "$CFG" "$DATA" |
  "$M10/canonicalize.py" > "$TMP/rust.json"
"$M10/compare.py" "$EXPECTED" "$TMP/rust.json"
if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  "$M10/compare.py" "$TMP/oracle.json" "$TMP/rust.json"
fi

"$TARGET/probe" "$DATA" "$CFG" |
  "$M10/canonicalize.py" > "$TMP/native.json"
"$M10/compare.py" "$EXPECTED" "$TMP/native.json"

echo 'M10 feature-value verification passed'
