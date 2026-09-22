#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M14="$(cd "$SCRIPT_DIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

(
  cd "$TMP"
  "$SCRIPT_DIR/run-cpp-oracle.sh" \
    "$M14/scenarios/02-hnsw-mutation-reopen.json"
) \
  | "$M14/canonicalize_behavior.py" \
  > "$TMP/mutation-reopen.json"

"$M14/compare.py" \
  "$M14/expected/hnsw-mutation-reopen.json" \
  "$TMP/mutation-reopen.json"

echo 'M14 mutation/reopen oracle regression passed'
