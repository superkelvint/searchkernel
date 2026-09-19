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

python3 - "$ORACLE" "$OBJ" "$OUT" "$COMPILE_DB" "$LINK_TXT" <<'PY'
import json
from pathlib import Path
import shlex
import subprocess
import sys

oracle = Path(sys.argv[1])
obj = Path(sys.argv[2])
out = Path(sys.argv[3])
compile_db = Path(sys.argv[4])
link_txt = Path(sys.argv[5])

db = json.loads(compile_db.read_text())
entry = next((x for x in db if str(x.get("file", "")).endswith("/documentdb_test.cpp")), None)
if entry is None:
    raise SystemExit("recursive grouping oracle: no documentdb_test.cpp compile command")
args = list(entry.get("arguments") or shlex.split(entry["command"]))
args.append("-Wno-inline")
old_source = str(entry["file"])
for i, arg in enumerate(args):
    if arg == old_source:
        args[i] = str(oracle)
        break
else:
    raise SystemExit("recursive grouping oracle: source not found in compile command")
out_i = args.index("-o")
args[out_i + 1] = str(obj)
subprocess.run(args, cwd=entry["directory"], check=True)

link = shlex.split(link_txt.read_text().strip())
suffix = "CMakeFiles/searchcore_documentdb_test_app.dir/documentdb_test.cpp.o"
for i, arg in enumerate(link):
    if arg.endswith(suffix):
        link[i] = str(obj)
        break
else:
    raise SystemExit("recursive grouping oracle: object not found in link command")
out_i = link.index("-o")
link[out_i + 1] = str(out)
subprocess.run(link, cwd=link_txt.parents[2], check=True)
PY

expected="$(expected_vespa_revision)"
printf 'vespa_commit=%s\n' "$expected" > "$OUT.provenance"
echo "recursive grouping oracle built: $OUT" >&2
