#!/usr/bin/env bash
set -euo pipefail
M12="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M12/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$M12/validate_scenarios.py"
"$M12/validate_expected.py"

# M12 is cumulative.
reference/vespa/m11/verify-m11.sh
run_workspace_quality
cargo check --manifest-path reference/vespa/api-contract/Cargo.toml --bin m12
"$M12/check-structure.sh"

[[ -x "$M12/oracle/target/m12_cpp_oracle" ]] || fail_verification \
  "M12 direct oracle is not built; author/review it per oracle/AUTHOR_ORACLE.md and run oracle/build-cpp-oracle.sh"

cargo build --quiet --manifest-path "$M12/rust-driver/Cargo.toml" --bins
LOW="$CARGO_TARGET_DIR/debug/searchkernel-m12-driver"
ADAPTER="$CARGO_TARGET_DIR/debug/searchkernel-m12-adapter-driver"
require_native_driver "$LOW" \
  sk_engine_open \
  sk_engine_search_generic \
  sk_generic_search_request_add_query_tensor_dense \
  sk_query_add_nearest_neighbor

DEFINITION_DIR="$(mktemp -d)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$DEFINITION_DIR"' EXIT
DEFINITION_BIN="$($M12/prepare-portable-definition.sh "$DEFINITION_DIR")"

for s in "$M12"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$s")"

  "$M12/oracle/run-cpp-oracle.sh" "$s" | "$M12/canonicalize.py" > "$TMP/oracle-$id.json"
  "$M12/compare.py" "$M12/expected/$id.json" "$TMP/oracle-$id.json"

  cfg="$TMP/$id-cfg"; data="$TMP/$id-data"
  "$M12/oracle/prepare-fixture.sh" "$s" "$cfg" "$data"
  cargo run --quiet --manifest-path "$M12/rust-driver/Cargo.toml" --bin searchkernel-m12-driver -- \
    "$s" "$cfg" "$data" | "$M12/canonicalize.py" > "$TMP/low-$id.json"
  "$M12/compare.py" "$M12/expected/$id.json" "$TMP/low-$id.json"
  "$M12/compare.py" "$TMP/oracle-$id.json" "$TMP/low-$id.json"

  # Re-prepare because opening/matching/close may mutate persistent engine state.
  rm -rf "$cfg" "$data"
  "$M12/oracle/prepare-fixture.sh" "$s" "$cfg" "$data"
  cargo run --quiet --manifest-path "$M12/rust-driver/Cargo.toml" --bin searchkernel-m12-adapter-driver -- \
    "$s" "$DEFINITION_BIN" "$cfg" "$data" | "$M12/canonicalize.py" > "$TMP/adapter-$id.json"
  "$M12/compare.py" "$M12/expected/$id.json" "$TMP/adapter-$id.json"
  "$M12/compare.py" "$TMP/oracle-$id.json" "$TMP/adapter-$id.json"
done

ERRORS="$M12/scenarios/errors.json"
# The negative driver must print exactly one expected_error category for every case.
while read -r id expected path; do
  cfg="$TMP/error-$id-cfg"; data="$TMP/error-$id-data"
  "$M12/oracle/prepare-fixture.sh" "$M12/scenarios/01-exact-near-far.json" "$cfg" "$data"
  actual="$(cargo run --quiet --manifest-path "$M12/rust-driver/Cargo.toml" --bin searchkernel-m12-error -- "$ERRORS" "$id" "$cfg" "$data" "$DEFINITION_BIN" 2>/dev/null || true)"
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
