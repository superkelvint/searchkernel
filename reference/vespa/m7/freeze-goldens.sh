#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$SCRIPT_DIR/validate_scenarios.py"
mkdir -p "$SCRIPT_DIR/expected"; rm -f "$SCRIPT_DIR/expected"/*.json
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 "$ROOT/tools/json-cli.py" get "$s" id)"
  raw="$(mktemp)"
  if ! "$SCRIPT_DIR/oracle/run-java-oracle.sh" "$s" > "$raw"; then echo "M7 Java oracle failed: $id" >&2; rm -f "$raw"; exit 1; fi
  python3 "reference/vespa/m7/freeze-goldens-python-1.py" "$raw" > "$SCRIPT_DIR/expected/$id.json"
  rm -f "$raw"
done
"$SCRIPT_DIR/validate_expected.py"
echo 'M7 goldens regenerated. Review expected/*.json, then commit the verifier specification.' >&2
