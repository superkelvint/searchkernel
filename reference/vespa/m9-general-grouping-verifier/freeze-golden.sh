#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa
"$DIR/validate.py"
[[ -x "$DIR/oracle/target/m9_recursive_grouping_oracle" ]] || \
  fail_verification "build and review the direct oracle first"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
"$DIR/oracle/run-cpp-oracle.sh" "$DIR/scenarios/01-recursive-two-level.json" \
  | "$DIR/canonicalize.py" > "$tmp"
cp "$tmp" "$DIR/expected/recursive-two-level.json"
echo "recursive grouping golden frozen from direct pinned-Vespa oracle" >&2
