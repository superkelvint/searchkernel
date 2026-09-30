#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 2 ]]; then
  echo "usage: $0 document|query OUTPUT.jsonl" >&2
  exit 2
fi
MODE="$1"
OUTPUT="$2"
[[ "$MODE" == document || "$MODE" == query ]] || { echo "invalid mode: $MODE" >&2; exit 2; }
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"

python3 "$M3/run_cases.py" \
  --cases "$M3/scenarios/cases.jsonl" \
  --output "$OUTPUT" \
  cargo run --quiet -p searchkernel-vespa-engine --example searchkernel-m3-conformance-driver -- "$MODE"
