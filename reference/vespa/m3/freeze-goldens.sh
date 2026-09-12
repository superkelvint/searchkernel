#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"

mkdir -p "$M3/expected"
"$M3/oracle/run-java-oracle.sh" document "$M3/expected/document.jsonl"
"$M3/oracle/run-java-oracle.sh" query "$M3/expected/query.jsonl"

echo "M3 goldens regenerated. Review expected/*.jsonl, then commit the verifier specification."
