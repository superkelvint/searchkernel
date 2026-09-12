#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"

if [[ "${1:-}" != "--force" ]] && { [[ -s "$M3/expected/document.jsonl" ]] || [[ -s "$M3/expected/query.jsonl" ]]; }; then
  echo "goldens already exist; use --force only while authoring the frozen harness" >&2
  exit 1
fi

mkdir -p "$M3/expected"
"$M3/oracle/run-java-oracle.sh" document "$M3/expected/document.jsonl"
"$M3/oracle/run-java-oracle.sh" query "$M3/expected/query.jsonl"

"$M3/freeze-harness.sh"
echo "M3 goldens and immutable checksum frozen. Review expected/*.jsonl before implementation starts."
