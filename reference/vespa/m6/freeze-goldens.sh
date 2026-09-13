#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$SCRIPT_DIR/validate_scenarios.py"

rm -rf "$SCRIPT_DIR/scenarios/rendered"
mkdir -p "$SCRIPT_DIR/scenarios/rendered" "$SCRIPT_DIR/expected"
rm -f "$SCRIPT_DIR/expected"/*.json

for scenario in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$scenario")"
  schema="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["schema"])' "$scenario")"
  sd="$SCRIPT_DIR/scenarios/rendered/$id.sd"
  "$SCRIPT_DIR/render_sd.py" "$scenario" "$sd"

  raw="$(mktemp)"
  if ! "$SCRIPT_DIR/oracle/run-java-oracle.sh" "$id" "$schema" "$sd" > "$raw"; then
    echo "M6 Java oracle failed for scenario: $id" >&2
    rm -f "$raw"
    exit 1
  fi
  python3 -c 'import json,sys; print(json.dumps(json.load(open(sys.argv[1])), sort_keys=True, separators=(",",":")))' "$raw" \
      > "$SCRIPT_DIR/expected/$id.json"
  rm -f "$raw"
done

"$SCRIPT_DIR/validate_expected.py"
"$SCRIPT_DIR/freeze-immutable.sh"
echo "M6 goldens regenerated. Review expected/*.json, then commit the verifier specification." >&2
