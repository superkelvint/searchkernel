#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"; cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
parse_verifier_args "$@"
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
reference/vespa/m6/verify-m6.sh --skip-workspace-quality
run_workspace_quality
cargo check --manifest-path reference/vespa/api-contract/Cargo.toml --bin m7
"$SCRIPT_DIR/check-structure.sh"
cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --bins
M7_DRIVER="$CARGO_TARGET_DIR/debug/searchkernel-m7-driver"
M7_ERROR="$CARGO_TARGET_DIR/debug/searchkernel-m7-error"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
 id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"
 if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
   "$SCRIPT_DIR/oracle/run-java-oracle.sh" "$s" | python3 -c 'import json,sys;print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))' > "$TMP/java-$id.json"
   "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$TMP/java-$id.json"
 fi
 run_native_executable "$M7_DRIVER" canonical "$s" > "$TMP/canon-$id.json"
 python3 - "$SCRIPT_DIR/expected/$id.json" "$TMP/canon-$id.json" <<'PY_CANON'
import json,sys
exp=json.load(open(sys.argv[1])); got=json.load(open(sys.argv[2]));
if got['id']!=exp['id'] or got['schema']!=exp['schema']: raise SystemExit('identity mismatch')
if got['typed']!=exp['query']: raise SystemExit(f"typed canonical mismatch: {got['typed']} != {exp['query']}")
if got['yql']!=exp['query']: raise SystemExit(f"YQL canonical mismatch: {got['yql']} != {exp['query']}")
PY_CANON
 for path in typed yql; do
   cfg="$TMP/$id-$path-cfg"; data="$TMP/$id-$path-data"; mkdir -p "$data"
   run_native_executable "$M7_DRIVER" execute "$s" "$cfg" "$data" "$path" > "$TMP/$id-$path-hits.json"
   "$SCRIPT_DIR/check_hits.py" "$s" "$TMP/$id-$path-hits.json"
 done
 "$SCRIPT_DIR/compare.py" "$TMP/$id-typed-hits.json" "$TMP/$id-yql-hits.json"
done
ERRORS="$SCRIPT_DIR/scenarios/errors.json"
python3 - "$ERRORS" <<'PY_JOBS' > "$TMP/error-jobs"
import json,sys
for c in json.load(open(sys.argv[1]))['cases']:
 for p in ('typed','yql'):
  if p in c: print(c['id'],p,c[p+'_error'])
PY_JOBS
while read -r id path expected; do
 cfg="$TMP/error-$id-$path-cfg"; data="$TMP/error-$id-$path-data"; mkdir -p "$data"
 actual="$(run_native_executable "$M7_ERROR" "$path" "$ERRORS" "$id" "$cfg" "$data")"
 [[ "$actual" == "$expected" ]] || { echo "M7 error category mismatch $id/$path: expected=$expected actual=$actual" >&2; exit 1; }
done < "$TMP/error-jobs"
echo 'M7 verification passed'
