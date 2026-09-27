#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
OUT="${1:?usage: run-rust.sh OUTPUT.jsonl}"
WORK="$M4/target/rust-run"
rm -rf "$WORK"
mkdir -p "$WORK"

export M4_INDEX_DIR="$WORK/index"
export M4_CONFIG_DIR="${M4_CONFIG_DIR:-$M4/fixture}"

source "$ROOT/reference/vespa/common/verify-common.sh"
cargo build --quiet --manifest-path "$M4/rust-driver/Cargo.toml"
python3 "$M4/run_scenario.py" \
  --mode rust \
  --scenario "$M4/scenarios/cases.json" \
  --output "$OUT" \
  "$ROOT/tools/run-runtime.sh" native "$CARGO_TARGET_DIR/debug/searchkernel-m4-driver"
