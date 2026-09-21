#!/usr/bin/env bash
set -euo pipefail
M14="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M14/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
require_sdk_manifest "$SDK/manifest.json"
"$M14/validate_scenarios.py"
"$M14/validate_expected.py"
python3 "$M14/test_execution_evidence.py"
python3 "$M14/test_symbol_expectations.py"

# M14 is cumulative.
"$ROOT/reference/vespa/m13/verify-m13.sh"
run_workspace_quality
cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m14
"$M14/check-structure.sh"

ORACLE="$M14/oracle/target/m14_cpp_oracle"
[[ -x "$ORACLE" ]] || fail_verification \
  'M14 direct oracle is not built; review oracle/AUTHOR_ORACLE.md and run oracle/build-cpp-oracle.sh'

cargo build --quiet --manifest-path "$M14/rust-driver/Cargo.toml" --bins
LOW="$CARGO_TARGET_DIR/debug/searchkernel-m14-driver"
ADAPTER="$CARGO_TARGET_DIR/debug/searchkernel-m14-adapter-driver"
ERROR="$CARGO_TARGET_DIR/debug/searchkernel-m14-error"
require_native_driver "$LOW" \
  sk_engine_open \
  sk_engine_put \
  sk_engine_update \
  sk_engine_remove \
  sk_generic_search_request_add_query_tensor_dense \
  sk_query_add_nearest_neighbor_with_options_handle
require_native_driver "$ADAPTER" \
  sk_engine_open \
  sk_engine_put \
  sk_engine_update \
  sk_engine_remove \
  sk_generic_search_request_add_query_tensor_dense \
  sk_query_add_nearest_neighbor_with_options_handle

TMP="$(mktemp -d)"
DEFINITION_DIR="$TMP/definition"
trap 'rm -rf "$TMP"' EXIT
DEFINITION_BIN="$($M14/prepare-portable-definition.sh "$DEFINITION_DIR")"

for scenario in "$M14"/scenarios/[0-9][0-9]-*.json; do
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$scenario")"

  "$M14/oracle/run-cpp-oracle.sh" "$scenario" \
    | "$M14/canonicalize_behavior.py" > "$TMP/$id-oracle.json"
  "$M14/compare.py" "$M14/expected/$id.json" "$TMP/$id-oracle.json"

  low_cfg="$TMP/$id-low-config"; low_data="$TMP/$id-low-data"
  low_trace="$TMP/$id-low-trace.jsonl"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    SEARCHKERNEL_NATIVE_TRACE_LEVEL=6 \
    SEARCHKERNEL_NATIVE_TRACE_FILE="$low_trace" \
    "$LOW" "$scenario" "$low_cfg" "$low_data" > "$TMP/$id-low-raw.json"
  python3 "$M14/validate_execution_evidence.py" "$scenario" "$TMP/$id-low-raw.json"
  "$M14/canonicalize_behavior.py" < "$TMP/$id-low-raw.json" > "$TMP/$id-low.json"
  "$M14/compare.py" "$M14/expected/$id.json" "$TMP/$id-low.json"
  "$M14/compare.py" "$TMP/$id-oracle.json" "$TMP/$id-low.json"
  "$M14/canonicalize_cfg.py" "$low_cfg" > "$TMP/$id-low-config.json"
  "$M14/compare.py" "$M14/expected/hnsw-config.json" "$TMP/$id-low-config.json"

  adapter_cfg="$TMP/$id-adapter-config"; adapter_data="$TMP/$id-adapter-data"
  adapter_trace="$TMP/$id-adapter-trace.jsonl"
  LD_LIBRARY_PATH="$SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    SEARCHKERNEL_NATIVE_TRACE_LEVEL=6 \
    SEARCHKERNEL_NATIVE_TRACE_FILE="$adapter_trace" \
    "$ADAPTER" "$scenario" "$DEFINITION_BIN" "$adapter_cfg" "$adapter_data" > "$TMP/$id-adapter-raw.json"
  python3 "$M14/validate_execution_evidence.py" "$scenario" "$TMP/$id-adapter-raw.json"
  "$M14/canonicalize_behavior.py" < "$TMP/$id-adapter-raw.json" > "$TMP/$id-adapter.json"
  "$M14/compare.py" "$M14/expected/$id.json" "$TMP/$id-adapter.json"
  "$M14/compare.py" "$TMP/$id-oracle.json" "$TMP/$id-adapter.json"
  "$M14/canonicalize_cfg.py" "$adapter_cfg" > "$TMP/$id-adapter-config.json"
  "$M14/compare.py" "$M14/expected/hnsw-config.json" "$TMP/$id-adapter-config.json"
done

# Negative schema cases. Portable cases are encoded through the real schema-aware FlatBuffers path.
ERRORS="$M14/scenarios/errors.json"
while read -r id path expected; do
  case_dir="$TMP/error-$id"; mkdir -p "$case_dir"
  definition="$DEFINITION_BIN"
  if [[ "$path" != low_level ]]; then
    json="$case_dir/definition.json"
    "$M14/make-error-definition.py" "$M14/fixture/index-definition.json" "$ERRORS" "$id" > "$json"
    mkdir -p "$case_dir/fb"
    "${FLATC_PATH:-flatc}" --binary --root-type SearchKernel.V2.IndexDefinition -o "$case_dir/fb" \
      "$ROOT/api/searchkernel.fbs" -- "$json"
    definition="$(find "$case_dir/fb" -maxdepth 1 -type f \( -name '*.bin' -o -name '*.bfbs' \) | head -1)"
  fi
  actual="$("$ERROR" "$ERRORS" "$id" "$definition" "$case_dir/config" 2>/dev/null || true)"
  [[ "$actual" == "$expected" ]] || {
    echo "M14 error category mismatch $id ($path): expected=$expected actual=${actual:-<empty>}" >&2
    exit 1
  }
done < <(python3 - "$ERRORS" <<'PY'
import json,sys
for case in json.load(open(sys.argv[1],encoding='utf-8'))['cases']:
    print(case['id'],case['path'],case['expected'])
PY
)

echo 'M14 HNSW approximate-NN verification passed'
