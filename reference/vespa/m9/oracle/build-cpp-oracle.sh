#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
M9="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="$(cd "$M9/../../.." && pwd)"

VESPA="$ROOT/upstream/vespa"
ORACLE="$M9/oracle/m9_attribute_grouping_oracle.cpp"
TARGET_DIR="$M9/oracle/target"
OUT="$TARGET_DIR/m9_cpp_oracle"
COMPILE_DB="$VESPA/compile_commands.json"
LINK_TXT="$VESPA/searchcore/src/tests/proton/documentdb/CMakeFiles/searchcore_documentdb_test_app.dir/link.txt"

source "$ROOT/reference/vespa/common/verify-common.sh"

die() {
    echo "M9 oracle build: $*" >&2
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

OBJ="$(mktemp "${TMPDIR:-/tmp}/searchkernel-m9-attribute-grouping-oracle.XXXXXX.o")"
trap 'rm -f "$OBJ"' EXIT

export M9_DIRECT_VESPA="$VESPA"
export M9_DIRECT_ORACLE="$ORACLE"
export M9_DIRECT_OBJ="$OBJ"
export M9_DIRECT_OUT="$OUT"
export M9_DIRECT_COMPILE_DB="$COMPILE_DB"
export M9_DIRECT_LINK_TXT="$LINK_TXT"

python3 - <<'PY'
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

vespa = Path(os.environ["M9_DIRECT_VESPA"])
oracle = Path(os.environ["M9_DIRECT_ORACLE"])
obj = Path(os.environ["M9_DIRECT_OBJ"])
out = Path(os.environ["M9_DIRECT_OUT"])
compile_db = Path(os.environ["M9_DIRECT_COMPILE_DB"])
link_txt = Path(os.environ["M9_DIRECT_LINK_TXT"])

db = json.loads(compile_db.read_text())

# Reuse Vespa's own DocumentDB unit-test translation unit as the compiler/link
# template. That target already carries the private Proton/searchlib libraries,
# generated config headers and compile definitions needed by a direct embedded
# DocumentDB oracle. We replace only documentdb_test.cpp and its object file.
entry = next(
    (x for x in db if str(x.get("file", "")).endswith("/documentdb_test.cpp")),
    None,
)
if entry is None:
    raise SystemExit(
        "M9 oracle build: no documentdb_test.cpp entry in compile_commands.json"
    )

if "arguments" in entry:
    compile_args = list(entry["arguments"])
else:
    compile_args = shlex.split(entry["command"])

# Vespa's test target enables -Winline, which reports non-actionable STL
# inlining decisions in otherwise clean oracle translation units. The native
# SDK build uses the same override; keep the direct oracle build quiet too.
compile_args.append("-Wno-inline")

old_source = str(entry["file"])
replaced_source = False
for i, arg in enumerate(compile_args):
    if arg == old_source:
        compile_args[i] = str(oracle)
        replaced_source = True
        break
if not replaced_source:
    raise SystemExit(
        "M9 oracle build: documentdb_test.cpp not found in compile command"
    )

try:
    out_i = compile_args.index("-o")
except ValueError:
    raise SystemExit("M9 oracle build: compile command has no -o") from None
compile_args[out_i + 1] = str(obj)

print("Compiling M9 direct Vespa oracle translation unit", file=sys.stderr)
subprocess.run(
    compile_args,
    cwd=entry["directory"],
    check=True,
)

link_args = shlex.split(link_txt.read_text().strip())
old_obj_suffix = (
    "CMakeFiles/searchcore_documentdb_test_app.dir/documentdb_test.cpp.o"
)

replaced_obj = False
for i, arg in enumerate(link_args):
    if arg.endswith(old_obj_suffix):
        link_args[i] = str(obj)
        replaced_obj = True
        break
if not replaced_obj:
    raise SystemExit(
        "M9 oracle build: documentdb_test.cpp.o not found in generated link command"
    )

try:
    out_i = link_args.index("-o")
except ValueError:
    raise SystemExit("M9 oracle build: link command has no -o") from None
link_args[out_i + 1] = str(out)

# CMake's generated link.txt is relative to the documentdb build directory:
#   .../documentdb/CMakeFiles/<target>.dir/link.txt
link_cwd = link_txt.parents[2]

print("Linking M9 oracle against pinned Vespa build artifacts", file=sys.stderr)
subprocess.run(
    link_args,
    cwd=link_cwd,
    check=True,
)

print(f"M9 oracle built: {out}", file=sys.stderr)
PY

[[ -x "$OUT" ]] || die "expected executable was not produced: $OUT"

# Provenance is intentionally adjacent to the binary so a golden-generation
# run can record which pinned Vespa checkout produced it.
{
    printf 'vespa_commit=%s\n' "$actual"
} > "$OUT.provenance"

printf 'M9 direct C++ oracle: %s\n' "$OUT" >&2
printf 'M9 provenance:        %s\n' "$OUT.provenance" >&2
