#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
EXPECTED="$M4/expected/results.jsonl"

# M3 must already have checked-in goldens before authoring M4.
[[ -s "$ROOT/reference/vespa/m3/expected/document.jsonl" && \
   -s "$ROOT/reference/vespa/m3/expected/query.jsonl" ]] || {
  echo "M3 goldens are missing; complete M3 before authoring M4" >&2
  exit 1
}

mkdir -p "$M4/expected"
"$M4/run-oracle.sh" "$EXPECTED"

# Sanity assertions on the oracle output before freezing it.
python3 "$M4/validate_expected.py" "$EXPECTED"
"$M4/freeze-immutable.sh"

echo "M4 goldens regenerated. Review expected/results.jsonl, then commit the verifier specification."
