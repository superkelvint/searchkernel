#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"; cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi
"$SCRIPT_DIR/validate_scenarios.py"
"$SCRIPT_DIR/validate_expected.py"
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do check_fixture_literals --keys text,value,yql "$s" "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" "$ROOT/core/native/vespa"; done
reference/vespa/m6/verify-m6.sh
run_workspace_quality
cargo check --manifest-path reference/vespa/api-contract/Cargo.toml --bin m7
"$SCRIPT_DIR/check-structure.sh"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
 id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"
 if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
   "$SCRIPT_DIR/oracle/run-java-oracle.sh" "$s" | python3 -c 'import json,sys;print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))' > "$TMP/java-$id.json"
   "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$TMP/java-$id.json"
 fi
 cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --bin searchkernel-m7-driver -- canonical "$s" > "$TMP/canon-$id.json"
 python3 "reference/vespa/m7/verify-m7-python-1.py" "$SCRIPT_DIR/expected/$id.json" "$TMP/canon-$id.json"
 for path in typed yql; do
   cfg="$TMP/$id-$path-cfg"; data="$TMP/$id-$path-data"; mkdir -p "$data"
   cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --bin searchkernel-m7-driver -- execute "$s" "$cfg" "$data" "$path" > "$TMP/$id-$path-hits.json"
   "$SCRIPT_DIR/check_hits.py" "$s" "$TMP/$id-$path-hits.json"
 done
 "$SCRIPT_DIR/compare.py" "$TMP/$id-typed-hits.json" "$TMP/$id-yql-hits.json"
done
ERRORS="$SCRIPT_DIR/scenarios/errors.json"
python3 "reference/vespa/m7/verify-m7-python-2.py" "$ERRORS"  > "$TMP/error-jobs"
while read -r id path expected; do
 cfg="$TMP/error-$id-$path-cfg"; data="$TMP/error-$id-$path-data"; mkdir -p "$data"
 actual="$(cargo run --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --bin searchkernel-m7-error -- "$path" "$ERRORS" "$id" "$cfg" "$data")"
 [[ "$actual" == "$expected" ]] || { echo "M7 error category mismatch $id/$path: expected=$expected actual=$actual" >&2; exit 1; }
done < "$TMP/error-jobs"
echo 'M7 verification passed'
