#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

here=$(c6_here)
root=$(c6_root)
flatc=$(find_flatc)
binary=$(bindgen_binary)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

copy_sources() {
  local destination=$1
  mkdir -p "$destination"
  cp "$root"/api/*.fbs "$destination"/
}

compile_and_remove_sources() {
  local source_dir=$1 bfbs_dir=$2
  generate_bfbs "$flatc" "$source_dir" "$bfbs_dir"
  find "$source_dir" -type f -name '*.fbs' -delete
}

expect_failure() {
  local label=$1 bfbs_dir=$2 expected=$3 detail=$4 output status
  if output=$("$binary" generate --bfbs-root "$bfbs_dir" --output-root "$tmp/$label-out" 2>&1); then
    echo "$label unexpectedly succeeded" >&2
    echo "$output" >&2
    exit 1
  else
    status=$?
  fi
  [[ "$status" -ne 0 ]]
  grep -F -- "$expected" <<<"$output" >/dev/null || {
    echo "$label did not report: $expected" >&2
    echo "$output" >&2
    exit 1
  }
  grep -F -- "$detail" <<<"$output" >/dev/null || {
    echo "$label did not identify: $detail" >&2
    echo "$output" >&2
    exit 1
  }
}

field_sources="$tmp/field-sources"
copy_sources "$field_sources"
python3 - "$field_sources/searchkernel.fbs" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
source = path.read_text()
start = source.index("table SearchRequest {")
end = source.index("\n}", start)
table = source[start:end]
needle = "  search_chain:string;"
if table.count(needle) != 1:
    raise SystemExit("could not locate SearchRequest field in schema")
table = table.replace(needle, needle + "\n  verifier_probe_field:string;")
path.write_text(source[:start] + table + source[end:])
PY
field_bfbs="$tmp/field-bfbs"
compile_and_remove_sources "$field_sources" "$field_bfbs"
expect_failure \
  unknown-field \
  "$field_bfbs" \
  "JSON field disposition inventory diverges from BFBS" \
  "SearchKernel.V2.SearchRequest.verifier_probe_field"

variant_sources="$tmp/variant-sources"
copy_sources "$variant_sources"
python3 - "$variant_sources/searchkernel.fbs" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
source = path.read_text()
needle = "  DeadlineExceeded = 10\n}"
replacement = "  DeadlineExceeded = 10,\n  VerifierSynthetic = 11\n}"
if source.count(needle) != 1:
    raise SystemExit("could not locate ErrorCode in schema")
path.write_text(source.replace(needle, replacement))
PY
variant_bfbs="$tmp/variant-bfbs"
compile_and_remove_sources "$variant_sources" "$variant_bfbs"
expect_failure \
  unknown-variant \
  "$variant_bfbs" \
  "JSON enum/union variant disposition inventory diverges from BFBS" \
  "SearchKernel.V2.ErrorCode.VerifierSynthetic"

orphan_sources="$tmp/orphan-sources"
copy_sources "$orphan_sources"
python3 - "$orphan_sources/searchkernel.fbs" "$orphan_sources/searchkernel_request.fbs" <<'PY'
from pathlib import Path
import sys

model_path = Path(sys.argv[1])
request_path = Path(sys.argv[2])
model = model_path.read_text()
request = request_path.read_text()

model_marker = "// -----------------------------------------------------------------------------\n// Search results\n// -----------------------------------------------------------------------------"
orphan_table = "table VerifierOrphanRequest {\n  value:string;\n}\n\n"
if model.count(model_marker) != 1:
    raise SystemExit("could not locate insertion point for orphan request")
model_path.write_text(model.replace(model_marker, orphan_table + model_marker))

needle = "  SearchKernel.V2.InspectQueryRequest\n}"
replacement = "  SearchKernel.V2.InspectQueryRequest,\n  SearchKernel.V2.VerifierOrphanRequest\n}"
if request.count(needle) != 1:
    raise SystemExit("could not locate RequestPayload in schema")
request_path.write_text(request.replace(needle, replacement))
PY
orphan_bfbs="$tmp/orphan-bfbs"
compile_and_remove_sources "$orphan_sources" "$orphan_bfbs"
expect_failure \
  orphan-request \
  "$orphan_bfbs" \
  "unpaired request operation" \
  "VerifierOrphanRequest"

malformed_bfbs="$tmp/malformed-bfbs"
mkdir -p "$malformed_bfbs"
cp "$root/core/crates/searchkernel-json/bfbs/searchkernel_request.bfbs" "$malformed_bfbs/"
cp "$root/core/crates/searchkernel-json/bfbs/searchkernel_response.bfbs" "$malformed_bfbs/"
truncate -s 8 "$malformed_bfbs/searchkernel_response.bfbs"
expect_failure malformed-bfbs "$malformed_bfbs" "invalid BFBS" "searchkernel_response.bfbs"

echo "C6 fail-closed BFBS mutation PASS"
