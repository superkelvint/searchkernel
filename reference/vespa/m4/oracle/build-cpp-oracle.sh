#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M4="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M4/../../.." && pwd)"

VESPA="$ROOT/upstream/vespa"
ORACLE="$M4/oracle/m4_text_oracle.cpp"
TARGET_DIR="$M4/oracle/target"
OUT="$TARGET_DIR/m4_cpp_oracle"
OBJ="${TMPDIR:-/tmp}/searchkernel-m4-text-oracle.o"

COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"
source "$ROOT/reference/vespa/common/verify-common.sh"

die() {
    echo "M4 oracle build: $*" >&2
    exit 1
}

[[ -f "$ORACLE" ]] || die "missing oracle source: $ORACLE"
[[ -f "$COMPILE_DB" ]] || die "missing compile_commands.json: $COMPILE_DB"
[[ -f "$LINK_TXT" ]] || die "missing generated link command: $LINK_TXT"
expected="$(expected_vespa_revision)"
require_pinned_vespa
actual="$expected"

mkdir -p "$TARGET_DIR"

export M4_DIRECT_VESPA="$VESPA"
export M4_DIRECT_ORACLE="$ORACLE"
export M4_DIRECT_OBJ="$OBJ"
export M4_DIRECT_OUT="$OUT"
export M4_DIRECT_COMPILE_DB="$COMPILE_DB"
export M4_DIRECT_LINK_TXT="$LINK_TXT"

python3 "reference/vespa/m4/oracle/build-cpp-oracle-python-1.py"

[[ -x "$OUT" ]] || die "expected executable was not produced: $OUT"

{
    printf 'vespa_commit=%s\n' "$actual"
} > "$OUT.provenance"
