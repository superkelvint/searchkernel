#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
WORK="$M4/target/verify"
mkdir -p "$WORK"
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

[[ -s "$M4/expected/results.jsonl" ]] || {
  echo "M4 frozen goldens are missing. Complete verifier authoring and run freeze-goldens.sh first." >&2
  exit 1
}
"$ROOT/reference/vespa/m3/verify-m3.sh" --skip-workspace-quality

run_workspace_quality

cargo check --quiet -p searchkernel-vespa-engine --example api-contract-m4

"$M4/run-rust.sh" "$WORK/rust-results.jsonl"
python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/rust-results.jsonl"
python3 "$M4/validate_expected.py" "$WORK/rust-results.jsonl"

"$M4/check-structure.sh"

if [[ "$RUN_LIVE_ORACLE" == 1 ]]; then
  "$M4/run-oracle.sh" "$WORK/cpp-results.jsonl"
  python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/cpp-results.jsonl"
  python3 "$M4/compare.py" "$WORK/cpp-results.jsonl" "$WORK/rust-results.jsonl"
fi

echo "M4 verification passed"
