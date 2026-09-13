#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
WORK="$M4/target/verify"
mkdir -p "$WORK"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa
require_immutable_checksum "$M4/M4-IMMUTABLE.sha256"

[[ -s "$M4/expected/results.jsonl" ]] || {
  echo "M4 frozen goldens are missing. Complete verifier authoring and run freeze-goldens.sh first." >&2
  exit 1
}
"$ROOT/reference/vespa/m3/verify-m3.sh"

run_workspace_quality

if [[ -f "$ROOT/reference/vespa/api-contract/Cargo.toml" ]]; then
  cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m4
fi

"$M4/run-rust.sh" "$WORK/rust-results.jsonl"
python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/rust-results.jsonl"
python3 "$M4/validate_expected.py" "$WORK/rust-results.jsonl"

"$M4/check-structure.sh"

# The direct Vespa C++ oracle is normative for M4 and is therefore mandatory.
"$M4/run-oracle.sh" "$WORK/cpp-results.jsonl"
python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/cpp-results.jsonl"
python3 "$M4/compare.py" "$WORK/cpp-results.jsonl" "$WORK/rust-results.jsonl"

echo "M4 verification passed"
