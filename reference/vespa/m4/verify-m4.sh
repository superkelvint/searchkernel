#!/usr/bin/env bash
set -euo pipefail
M4="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"
WORK="$M4/target/verify"
mkdir -p "$WORK"
cd "$ROOT"

[[ -s "$M4/expected/results.jsonl" ]] || {
  echo "M4 frozen goldens are missing. Complete verifier authoring and run freeze-goldens.sh first." >&2
  exit 1
}
# M4 is cumulative through M3.
if [[ -x "$ROOT/reference/vespa/m3/verify-m3.sh" ]]; then
  "$ROOT/reference/vespa/m3/verify-m3.sh"
else
  echo "cannot locate cumulative M3 verifier" >&2
  exit 1
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features

if [[ -f "$ROOT/reference/vespa/api-contract/Cargo.toml" ]]; then
  cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m4
fi

"$M4/run-rust.sh" "$WORK/rust-results.jsonl"
python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/rust-results.jsonl"
python3 "$M4/validate_expected.py" "$WORK/rust-results.jsonl"

"$M4/check-structure.sh"

# Optional independent oracle-drift check for acceptance environments that have
# the full direct-C++ oracle build available. Frozen goldens remain mandatory.
if [[ "${M4_LIVE_ORACLE:-0}" == "1" ]]; then
  "$M4/run-oracle.sh" "$WORK/cpp-results.jsonl"
  python3 "$M4/compare.py" "$M4/expected/results.jsonl" "$WORK/cpp-results.jsonl"
  python3 "$M4/compare.py" "$WORK/cpp-results.jsonl" "$WORK/rust-results.jsonl"
fi

echo "M4 verification passed"
