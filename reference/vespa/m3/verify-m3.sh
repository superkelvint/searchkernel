#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
M3="$ROOT/reference/vespa/m3"
WORK="$M3/target/verify"
mkdir -p "$WORK"
cd "$ROOT"
source "$ROOT/reference/vespa/common/verify-common.sh"

require_pinned_vespa

[[ -s "$M3/expected/document.jsonl" && -s "$M3/expected/query.jsonl" ]] || {
  echo "M3 frozen goldens are missing. Harness author must run freeze-goldens.sh before implementation." >&2
  exit 1
}
"$ROOT/reference/vespa/m2/verify-m2.sh"

"$M3/kstem/verify-kstem.sh"

run_workspace_quality

if [[ -f "$ROOT/reference/vespa/api-contract/Cargo.toml" ]]; then
  cargo check --quiet --manifest-path "$ROOT/reference/vespa/api-contract/Cargo.toml" --bin m3
fi

"$M3/oracle/run-java-oracle.sh" document "$WORK/java-document.jsonl"
"$M3/oracle/run-java-oracle.sh" query "$WORK/java-query.jsonl"
"$M3/run-rust.sh" document "$WORK/rust-document.jsonl"
"$M3/run-rust.sh" query "$WORK/rust-query.jsonl"

# Oracle drift guard.
python3 "$M3/compare.py" "$M3/expected/document.jsonl" "$WORK/java-document.jsonl"
python3 "$M3/compare.py" "$M3/expected/query.jsonl" "$WORK/java-query.jsonl"

# Rust vs frozen contract.
python3 "$M3/compare.py" "$M3/expected/document.jsonl" "$WORK/rust-document.jsonl"
python3 "$M3/compare.py" "$M3/expected/query.jsonl" "$WORK/rust-query.jsonl"

# Direct live parity against pinned Vespa Java.
python3 "$M3/compare.py" "$WORK/java-document.jsonl" "$WORK/rust-document.jsonl"
python3 "$M3/compare.py" "$WORK/java-query.jsonl" "$WORK/rust-query.jsonl"

"$M3/check-structure.sh"

echo "M3 verification passed"
