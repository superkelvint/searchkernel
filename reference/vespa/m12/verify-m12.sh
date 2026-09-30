#!/usr/bin/env bash
set -euo pipefail
M12="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M12/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
parse_verifier_args "$@"
require_pinned_vespa
RUN_LIVE_ORACLE=0
if live_oracle_enabled; then
  RUN_LIVE_ORACLE=1
else
  announce_frozen_oracle_replay
fi
"$M12/validate_scenarios.py"
"$M12/validate_expected.py"

# M12 is cumulative.
reference/vespa/m11/verify-m11.sh --skip-workspace-quality
run_workspace_quality
cargo check -p searchkernel-vespa-engine --example api-contract-m12
"$M12/check-structure.sh"

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  [[ -x "$M12/oracle/target/m12_cpp_oracle" ]] || fail_verification \
    "M12 direct oracle is not built; author/review it per oracle/AUTHOR_ORACLE.md and run oracle/build-cpp-oracle.sh"
fi

cargo build --quiet -p searchkernel --features native --examples
LOW="$CARGO_TARGET_DIR/debug/examples/searchkernel-m12-driver"
ADAPTER="$CARGO_TARGET_DIR/debug/examples/searchkernel-m12-adapter-driver"
PREPARE="$CARGO_TARGET_DIR/debug/examples/searchkernel-m12-prepare-fixture"
require_native_driver "$LOW" \
  sk_engine_open \
  sk_engine_search_generic \
  sk_generic_search_request_add_query_tensor_dense \
  sk_query_add_nearest_neighbor
require_native_driver "$PREPARE" sk_engine_open sk_engine_put

prepare_fixture() {
  local scenario=$1
  local config=$2
  local data=$3
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M12/oracle/prepare-fixture.sh" "$scenario" "$config" "$data"
  else
    "$PREPARE" "$scenario" "$config" "$data"
  fi
}

DEFINITION_DIR="$(mktemp -d)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$DEFINITION_DIR"' EXIT
DEFINITION_BIN="$($M12/prepare-portable-definition.sh "$DEFINITION_DIR")"

for s in "$M12"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"

  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M12/oracle/run-cpp-oracle.sh" "$s" | "$M12/canonicalize.py" > "$TMP/oracle-$id.json"
    "$M12/compare.py" "$M12/expected/$id.json" "$TMP/oracle-$id.json"
  fi

  cfg="$TMP/$id-cfg"; data="$TMP/$id-data"
  prepare_fixture "$s" "$cfg" "$data"
  cargo run --quiet -p searchkernel --features native --example searchkernel-m12-driver -- \
    "$s" "$cfg" "$data" | "$M12/canonicalize.py" > "$TMP/low-$id.json"
  "$M12/compare.py" "$M12/expected/$id.json" "$TMP/low-$id.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M12/compare.py" "$TMP/oracle-$id.json" "$TMP/low-$id.json"
  fi

  # Re-prepare because opening/matching/close may mutate persistent engine state.
  rm -rf "$cfg" "$data"
  prepare_fixture "$s" "$cfg" "$data"
  cargo run --quiet -p searchkernel --features native --example searchkernel-m12-adapter-driver -- \
    "$s" "$DEFINITION_BIN" "$cfg" "$data" | "$M12/canonicalize.py" > "$TMP/adapter-$id.json"
  "$M12/compare.py" "$M12/expected/$id.json" "$TMP/adapter-$id.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M12/compare.py" "$TMP/oracle-$id.json" "$TMP/adapter-$id.json"
  fi
done

ERRORS="$M12/scenarios/errors.json"
# The negative driver must print exactly one expected_error category for every case.
while read -r id expected path; do
  cfg="$TMP/error-$id-cfg"; data="$TMP/error-$id-data"
  prepare_fixture "$M12/scenarios/01-exact-near-far.json" "$cfg" "$data"
  actual="$(cargo run --quiet -p searchkernel --features native --example searchkernel-m12-error -- "$ERRORS" "$id" "$cfg" "$data" "$DEFINITION_BIN" 2>/dev/null || true)"
  [[ "$actual" == "$expected" ]] || {
    echo "M12 error category mismatch $id ($path): expected=$expected actual=${actual:-<empty>}" >&2
    exit 1
  }
done < <(python3 - "$ERRORS" <<'PY'
import json,sys
for c in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
    print(c['id'],c['expected_error'],c['path'])
PY
)

echo 'M12 verification passed'
