#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M10="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M10/../../.." && pwd)"

VESPA="$ROOT/upstream/vespa"
ORACLE="$M10/oracle/m10_ranking_oracle.cpp"
TARGET_DIR="$M10/oracle/target"
OUT="$TARGET_DIR/m10_cpp_oracle"
COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"

source "$ROOT/reference/vespa/common/verify-common.sh"

die() {
    echo "M10 oracle build: $*" >&2
    exit 1
}

[[ -f "$ORACLE" ]] || die "missing oracle source: $ORACLE"
[[ -f "$COMPILE_DB" ]] || die "missing compile_commands.json: $COMPILE_DB"
[[ -f "$LINK_TXT" ]] || die "missing generated link command: $LINK_TXT"

# The direct oracle must be compiled
# against exactly the pinned Vespa checkout, never an ambient SDK or another
# checkout. require_pinned_vespa also rejects a dirty upstream tree.
expected="$(expected_vespa_revision)"
require_pinned_vespa
actual="$(git -c safe.directory="$VESPA" -C "$VESPA" rev-parse HEAD)"
[[ "$actual" == "$expected" ]] || die \
    "pinned Vespa mismatch after verification: expected $expected, got $actual"

mkdir -p "$TARGET_DIR"

OBJ="$(mktemp "${TMPDIR:-/tmp}/searchkernel-m10-ranking-oracle.XXXXXX.o")"
trap 'rm -f "$OBJ"' EXIT

export M10_DIRECT_VESPA="$VESPA"
export M10_DIRECT_ORACLE="$ORACLE"
export M10_DIRECT_OBJ="$OBJ"
export M10_DIRECT_OUT="$OUT"
export M10_DIRECT_COMPILE_DB="$COMPILE_DB"
export M10_DIRECT_LINK_TXT="$LINK_TXT"

python3 "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/build-cpp-oracle-python-1.py"

[[ -x "$OUT" ]] || die "expected executable was not produced: $OUT"

# Provenance is intentionally adjacent to the binary so a golden-generation
# run can record which pinned Vespa checkout produced it.
{
    printf 'vespa_commit=%s\n' "$actual"
} > "$OUT.provenance"

printf 'M10 direct C++ oracle: %s\n' "$OUT" >&2
printf 'M10 provenance:        %s\n' "$OUT.provenance" >&2
