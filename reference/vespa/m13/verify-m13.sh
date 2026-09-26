#!/usr/bin/env bash
set -euo pipefail

M13="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M13/../../.." && pwd)"
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
require_sdk_manifest "$SDK/manifest.json"

# M13 is cumulative: the previous dense-query milestone must remain green.
"$ROOT/reference/vespa/m12/verify-m12.sh" --skip-workspace-quality
run_workspace_quality

cargo fmt --all --check
cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m13

for symbol in \
    sk_document_add_tensor_dense \
    sk_document_update_assign_tensor_dense \
    sk_document_field_tensor_dense \
    sk_engine_get \
    sk_engine_update \
    sk_generic_search_request_add_query_tensor_dense \
    sk_query_add_nearest_neighbor; do
  for file in \
      "$ROOT/core/native/vespa/searchkernel_vespa.h" \
      "$ROOT/core/native/vespa/searchkernel_vespa.map" \
      "$ROOT/core/crates/searchkernel-vespa-bindings/src/bindings.rs"; do
    rg -n "$symbol" "$file" >/dev/null ||
      fail_verification "M13 native tensor symbol is missing from $file: $symbol"
  done
done

if rg -n 'FieldValue::Tensor\(_\)\s*=>\s*Value::Null|shadow state|fake tensor database' \
    "$ROOT/core/crates/searchkernel-ir-adapter" "$ROOT/core/crates/searchkernel-vespa-engine"; then
  fail_verification 'M13 contains a lossy adapter path or parallel tensor state'
fi
if rg -n 'serde_json.*tensor.*(store|database)|sort_by.*document_id|fake.*NN' \
    "$M13/rust-driver"; then
  fail_verification 'M13 verifier contains a tensor storage or fake-NN shortcut'
fi

cargo build --quiet --manifest-path "$M13/rust-driver/Cargo.toml" --bins
LOW="$CARGO_TARGET_DIR/debug/searchkernel-m13-driver"
ADAPTER="$CARGO_TARGET_DIR/debug/searchkernel-m13-adapter-driver"
ERROR="$CARGO_TARGET_DIR/debug/searchkernel-m13-error"
ORACLE="$M13/oracle/target/m13_cpp_oracle"
if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  [[ -x "$ORACLE" ]] || fail_verification \
    "M13 direct oracle is not built; run oracle/build-cpp-oracle.sh"
fi
require_native_driver "$LOW" \
  sk_document_add_tensor_dense \
  sk_document_update_assign_tensor_dense \
  sk_document_field_tensor_dense \
  sk_engine_get \
  sk_engine_update \
  sk_generic_search_request_add_query_tensor_dense \
  sk_query_add_nearest_neighbor
require_native_driver "$ADAPTER" \
  sk_document_update_assign_tensor_dense \
  sk_document_field_tensor_dense
require_native_driver "$ERROR" \
  sk_document_add_tensor_dense \
  sk_document_update_assign_tensor_dense \
  sk_document_field_tensor_dense

DEFINITION_DIR="$(mktemp -d)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP" "$DEFINITION_DIR"' EXIT
DEFINITION_BIN="$($M13/prepare-portable-definition.sh "$DEFINITION_DIR")"

for scenario in "$M13"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$scenario")"

  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M13/oracle/run-cpp-oracle.sh" "$scenario" >"$TMP/$id-oracle.json"
    "$M13/compare.py" "$M13/expected/$id.json" <"$TMP/$id-oracle.json"
  fi

  low_config="$TMP/$id-low-config"
  low_data="$TMP/$id-low-data"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$LOW" "$scenario" "$low_config" "$low_data" >"$TMP/$id-low.json"
  "$M13/compare.py" "$M13/expected/$id.json" <"$TMP/$id-low.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M13/compare.py" "$TMP/$id-oracle.json" <"$TMP/$id-low.json"
  fi

  adapter_config="$TMP/$id-adapter-config"
  adapter_data="$TMP/$id-adapter-data"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$ADAPTER" "$scenario" "$DEFINITION_BIN" "$adapter_config" "$adapter_data" \
    >"$TMP/$id-adapter.json"
  "$M13/compare.py" "$M13/expected/$id.json" <"$TMP/$id-adapter.json"
  if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
    "$M13/compare.py" "$TMP/$id-oracle.json" <"$TMP/$id-adapter.json"
  fi
done

ERRORS="$M13/scenarios/errors.json"
while read -r id expected path; do
  config="$TMP/error-$id-config"
  data="$TMP/error-$id-data"
  actual="$(LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$ERROR" "$ERRORS" "$id" "$DEFINITION_BIN" "$config" "$data" 2>/dev/null || true)"
  [[ "$actual" == "$expected" ]] || {
    echo "M13 error category mismatch $id ($path): expected=$expected actual=${actual:-<empty>}" >&2
    exit 1
  }
done < <(python3 - "$ERRORS" <<'PY'
import json,sys
for case in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
    print(case['id'],case['expected'],case['path'])
PY
)

echo 'M13 dense tensor document CRUD verification passed'
