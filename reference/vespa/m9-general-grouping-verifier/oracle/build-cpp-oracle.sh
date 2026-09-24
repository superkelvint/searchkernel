#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
VERIFIER="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$VERIFIER/../../.." && pwd)"
VESPA="$ROOT/upstream/vespa"
ORACLE="$SCRIPT_DIR/m9_recursive_grouping_oracle.cpp"
TARGET_DIR="$SCRIPT_DIR/target"
OUT="$TARGET_DIR/m9_recursive_grouping_oracle"
COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"

source "$ROOT/reference/vespa/common/verify-common.sh"
require_pinned_vespa

[[ -f "$ORACLE" ]] || fail_verification "missing oracle source: $ORACLE"
[[ -f "$COMPILE_DB" ]] || fail_verification "missing Vespa compile_commands.json"
[[ -f "$LINK_TXT" ]] || fail_verification "missing Vespa DocumentDB link command"
mkdir -p "$TARGET_DIR"

OBJ="$(mktemp "${TMPDIR:-/tmp}/searchkernel-recursive-grouping.XXXXXX.o")"
trap 'rm -f "$OBJ"' EXIT

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/build-cpp-oracle-python-1.py" "$ORACLE" "$OBJ" "$OUT" "$COMPILE_DB" "$LINK_TXT"

expected="$(expected_vespa_revision)"
printf 'vespa_commit=%s\n' "$expected" > "$OUT.provenance"
echo "recursive grouping oracle built: $OUT" >&2
