#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
OUT="${1:?usage: run-oracle.sh OUTPUT.jsonl}"

python3 "$M4/run_scenario.py" \
  --mode oracle \
  --scenario "$M4/scenarios/cases.json" \
  --output "$OUT" \
  "$M4/oracle/run-cpp-oracle.sh"
