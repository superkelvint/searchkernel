#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
EXPECTED="$M4/expected/results.jsonl"

if [[ "${1:-}" != "--force" && -s "$EXPECTED" ]]; then
  echo "M4 goldens already exist; --force is allowed only while authoring the harness" >&2
  exit 1
fi

# M3 must already be frozen and authoritative before authoring M4.
[[ -s "$ROOT/reference/vespa/m3/M3-IMMUTABLE.sha256" ]] || {
  echo "M3 is not frozen; complete M3 before freezing M4" >&2
  exit 1
}
sha256sum -c "$ROOT/reference/vespa/m3/M3-IMMUTABLE.sha256"

mkdir -p "$M4/expected"
"$M4/run-oracle.sh" "$EXPECTED"

# Sanity assertions on the oracle output before freezing it.
python3 "$M4/validate_expected.py" "$EXPECTED"
"$M4/freeze-harness.sh"

echo "M4 goldens and immutable checksum frozen. Review expected/results.jsonl before implementation starts."
