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
"$M3/oracle/build-java-oracle.sh"
BUILD="$M3/oracle/target"
CP="$BUILD/classes:$(cat "$BUILD/runtime-classpath.txt")"

python3 "$M3/run_cases.py" \
  --cases "$M3/scenarios/cases.jsonl" \
  --output "$OUTPUT" \
  java -cp "$CP" searchkernel.m3.LinguisticsOracle "$MODE"
