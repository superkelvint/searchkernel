#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M14="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M14/../../.." && pwd)"
VESPA="$ROOT/upstream/vespa"
ORACLE="$SCRIPT_DIR/m14_hnsw_oracle.cpp"
TARGET_DIR="$SCRIPT_DIR/target"
OUT="$TARGET_DIR/m14_cpp_oracle"
COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"
source "$ROOT/reference/vespa/common/verify-common.sh"
die(){ echo "M14 oracle build: $*" >&2; exit 1; }
[[ -f "$ORACLE" ]] || die "missing reviewed oracle source: $ORACLE (see AUTHOR_ORACLE.md)"
[[ -f "$COMPILE_DB" ]] || die "missing compile_commands.json: $COMPILE_DB"
[[ -f "$LINK_TXT" ]] || die "missing generated DocumentDB link command: $LINK_TXT"
require_pinned_vespa
mkdir -p "$TARGET_DIR"
OBJ="$(mktemp "${TMPDIR:-/tmp}/searchkernel-m14-hnsw-oracle.XXXXXX.o")"
trap 'rm -f "$OBJ"' EXIT
export M14_DIRECT_VESPA="$VESPA" M14_DIRECT_ORACLE="$ORACLE" M14_DIRECT_OBJ="$OBJ" M14_DIRECT_OUT="$OUT" M14_DIRECT_COMPILE_DB="$COMPILE_DB" M14_DIRECT_LINK_TXT="$LINK_TXT"
python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/build-cpp-oracle-python-1.py"
[[ -x "$OUT" ]] || die "oracle executable was not produced"
printf 'vespa_commit=%s\n' "$(expected_vespa_revision)" > "$OUT.provenance"
echo "M14 direct C++ oracle: $OUT" >&2
