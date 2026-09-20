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
python3 - <<'PY'
import json, os, shlex, subprocess
from pathlib import Path
vespa=Path(os.environ['M13_DIRECT_VESPA']); oracle=Path(os.environ['M13_DIRECT_ORACLE'])
obj=Path(os.environ['M13_DIRECT_OBJ']); out=Path(os.environ['M13_DIRECT_OUT'])
db=json.loads(Path(os.environ['M13_DIRECT_COMPILE_DB']).read_text())
link_txt=Path(os.environ['M13_DIRECT_LINK_TXT'])
entry=next((x for x in db if str(x.get('file','')).endswith('/documentdb_test.cpp')),None)
if entry is None: raise SystemExit('M13 oracle build: missing documentdb_test.cpp compile entry')
args=list(entry['arguments']) if 'arguments' in entry else shlex.split(entry['command'])
old=str(entry['file'])
build_prefix='/work/upstream/vespa'
actual_prefix=str(vespa.resolve())
deps_prefix='/opt/vespa-deps'
actual_deps=str((vespa.parent.parent / 'vespa-deps').resolve())
args=[arg.replace(build_prefix, actual_prefix) for arg in args]
args=[arg.replace(deps_prefix, actual_deps) for arg in args]
if args and args[0].endswith('/ccache') and not Path(args[0]).exists():
    args=args[1:]
if args and not Path(args[0]).exists():
    args[0]='c++'
old=old.replace(build_prefix, actual_prefix)
args[args.index(old)]=str(oracle)
args[args.index('-o')+1]=str(obj)
args.append('-Wno-inline')
compile_cwd=entry['directory'].replace(build_prefix, actual_prefix)
subprocess.run(args,cwd=compile_cwd,check=True)
link=shlex.split(link_txt.read_text().strip())
link=[arg.replace(build_prefix, actual_prefix) for arg in link]
link=[arg.replace(deps_prefix, actual_deps) for arg in link]
if link and link[0].endswith('/ccache') and not Path(link[0]).exists():
    link=link[1:]
if link and not Path(link[0]).exists():
    link[0]='c++'
suffix='CMakeFiles/searchcore_documentdb_test_app.dir/documentdb_test.cpp.o'
for i,arg in enumerate(link):
    if arg.endswith(suffix): link[i]=str(obj); break
else: raise SystemExit('M13 oracle build: documentdb object not found in link command')
link[link.index('-o')+1]=str(out)
subprocess.run(link,cwd=str(vespa / 'searchcore/src/tests/proton/documentdb'),check=True)
PY
[[ -x "$OUT" ]] || fail_verification "M13 oracle executable was not produced"
printf 'vespa_commit=%s\n' "$(expected_vespa_revision)" > "$OUT.provenance"
