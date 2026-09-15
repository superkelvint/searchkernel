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

python3 - <<'PY'
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

vespa = Path(os.environ["M4_DIRECT_VESPA"])
oracle = Path(os.environ["M4_DIRECT_ORACLE"])
obj = Path(os.environ["M4_DIRECT_OBJ"])
out = Path(os.environ["M4_DIRECT_OUT"])
compile_db = Path(os.environ["M4_DIRECT_COMPILE_DB"])
link_txt = Path(os.environ["M4_DIRECT_LINK_TXT"])

db = json.loads(compile_db.read_text())

entry = next(
    (x for x in db if str(x.get("file", "")).endswith("/documentdb_test.cpp")),
    None,
)
if entry is None:
    raise SystemExit("M4 oracle build: no documentdb_test.cpp entry in compile_commands.json")

if "arguments" in entry:
    compile_args = list(entry["arguments"])
else:
    compile_args = shlex.split(entry["command"])

old_source = str(entry["file"])
compile_args = [
    str(oracle) if arg == old_source else arg
    for arg in compile_args
]

try:
    out_i = compile_args.index("-o")
except ValueError:
    raise SystemExit("M4 oracle build: compile command has no -o") from None

compile_args[out_i + 1] = str(obj)

print("Compiling M4 oracle translation unit", file=sys.stderr)
subprocess.run(
    compile_args,
    cwd=entry["directory"],
    check=True,
)

link_args = shlex.split(link_txt.read_text().strip())

old_obj_suffix = (
    "CMakeFiles/searchcore_documentdb_test_app.dir/documentdb_test.cpp.o"
)

replaced = False
for i, arg in enumerate(link_args):
    if arg.endswith(old_obj_suffix):
        link_args[i] = str(obj)
        replaced = True
        break

if not replaced:
    raise SystemExit(
        "M4 oracle build: documentdb_test.cpp.o not found in generated link command"
    )

try:
    out_i = link_args.index("-o")
except ValueError:
    raise SystemExit("M4 oracle build: link command has no -o") from None

link_args[out_i + 1] = str(out)

# CMake generated link.txt expecting to run from the documentdb build directory:
# .../documentdb/CMakeFiles/<target>.dir/link.txt
link_cwd = link_txt.parents[2]

print("Linking M4 oracle against existing Vespa artifacts", file=sys.stderr)
subprocess.run(
    link_args,
    cwd=link_cwd,
    check=True,
)

print(f"M4 oracle built: {out}", file=sys.stderr)
PY

[[ -x "$OUT" ]] || die "expected executable was not produced: $OUT"

{
    printf 'vespa_commit=%s\n' "$actual"
} > "$OUT.provenance"
