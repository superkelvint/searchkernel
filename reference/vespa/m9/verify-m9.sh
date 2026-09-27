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
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
  check_fixture_literals --keys string "$s" "$ROOT/core/crates/searchkernel-vespa-engine/src" "$ROOT/core/crates/searchkernel-vespa-bindings/src" "$ROOT/core/native/vespa"
done
reference/vespa/m8/verify-m8.sh --skip-workspace-quality
run_workspace_quality
cargo check --manifest-path reference/vespa/api-contract/Cargo.toml --bin m9
"$SCRIPT_DIR/check-structure.sh"

cargo build --quiet --manifest-path "$SCRIPT_DIR/rust-driver/Cargo.toml" --bins
require_native_driver "$CARGO_TARGET_DIR/debug/searchkernel-m9-driver" \
  sk_engine_open \
  sk_engine_group_tree \
  sk_engine_update

M9_DRIVER="$CARGO_TARGET_DIR/debug/searchkernel-m9-driver"
M9_ERROR="$CARGO_TARGET_DIR/debug/searchkernel-m9-error"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
for s in "$SCRIPT_DIR"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$SCRIPT_DIR/oracle/run-cpp-oracle.sh" "$s" | "$SCRIPT_DIR/canonicalize.py" > "$TMP/oracle-$id.json"
    "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$TMP/oracle-$id.json"
  fi
  cfg="$TMP/$id-cfg"; data="$TMP/$id-data"; mkdir -p "$data"
  run_native_executable "$M9_DRIVER" "$s" "$cfg" "$data" | "$SCRIPT_DIR/canonicalize.py" > "$TMP/rust-$id.json"
  "$SCRIPT_DIR/compare.py" "$SCRIPT_DIR/expected/$id.json" "$TMP/rust-$id.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$SCRIPT_DIR/compare.py" "$TMP/oracle-$id.json" "$TMP/rust-$id.json"
  fi
done

ERRORS="$SCRIPT_DIR/scenarios/errors.json"
python3 - "$ERRORS" <<'PY' > "$TMP/error-jobs"
import json,sys
for c in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
 print(c['id'],c['expected_error'])
PY
while read -r id expected; do
  cfg="$TMP/error-$id-cfg"; data="$TMP/error-$id-data"; mkdir -p "$data"
  actual="$(run_native_executable "$M9_ERROR" "$ERRORS" "$id" "$cfg" "$data")"
  [[ "$actual" == "$expected" ]] || { echo "M9 error category mismatch $id: expected=$expected actual=$actual" >&2; exit 1; }
done < "$TMP/error-jobs"

reference/vespa/m9-general-grouping-verifier/verify-recursive-grouping.sh

echo 'M9 verification passed'
