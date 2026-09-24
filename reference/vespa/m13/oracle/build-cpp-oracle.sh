#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M13="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M13/../../.." && pwd)"
VESPA="$ROOT/upstream/vespa"
ORACLE="$SCRIPT_DIR/m13_tensor_oracle.cpp"
TARGET_DIR="$SCRIPT_DIR/target"
OUT="$TARGET_DIR/m13_cpp_oracle"
COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"
source "$ROOT/reference/vespa/common/verify-common.sh"
[[ -f "$COMPILE_DB" ]] || fail_verification "missing Vespa compile_commands.json"
[[ -f "$LINK_TXT" ]] || fail_verification "missing generated DocumentDB link command"
require_pinned_vespa
mkdir -p "$TARGET_DIR"
OBJ="$(mktemp "${TMPDIR:-/tmp}/searchkernel-m13-oracle.XXXXXX.o")"
trap 'rm -f "$OBJ"' EXIT
export M13_DIRECT_VESPA="$VESPA" M13_DIRECT_ORACLE="$ORACLE" M13_DIRECT_OBJ="$OBJ" M13_DIRECT_OUT="$OUT" M13_DIRECT_COMPILE_DB="$COMPILE_DB" M13_DIRECT_LINK_TXT="$LINK_TXT"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/build-cpp-oracle-python-1.py"
[[ -x "$OUT" ]] || fail_verification "M13 oracle executable was not produced"
printf 'vespa_commit=%s\n' "$(expected_vespa_revision)" > "$OUT.provenance"
