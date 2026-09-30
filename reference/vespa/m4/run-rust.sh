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

python3 "$M4/run_scenario.py" \
  --mode rust \
  --scenario "$M4/scenarios/cases.json" \
  --output "$OUT" \
  cargo run --quiet -p searchkernel-vespa-engine --example searchkernel-m4-driver --
