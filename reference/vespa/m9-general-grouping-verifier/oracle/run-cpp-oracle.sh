#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
VERIFIER="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$VERIFIER/../../.." && pwd)"
source "$ROOT/reference/vespa/common/verify-common.sh"
SCENARIO="${1:?usage: run-cpp-oracle.sh SCENARIO_JSON}"
BIN="$SCRIPT_DIR/target/m9_recursive_grouping_oracle"
[[ -x "$BIN" ]] || fail_verification "build oracle first: $SCRIPT_DIR/build-cpp-oracle.sh"
export M9_ORACLE_FIXTURE_ROOT="$ROOT/reference/vespa/m9/fixture"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"
exec "$BIN" "$(cd "$(dirname "$SCENARIO")" && pwd -P)/$(basename "$SCENARIO")"
