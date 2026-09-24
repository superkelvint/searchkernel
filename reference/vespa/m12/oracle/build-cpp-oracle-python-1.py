#!/usr/bin/env python3
import json, os, shlex, subprocess
from pathlib import Path
vespa=Path(os.environ['M12_DIRECT_VESPA']); oracle=Path(os.environ['M12_DIRECT_ORACLE'])
obj=Path(os.environ['M12_DIRECT_OBJ']); out=Path(os.environ['M12_DIRECT_OUT'])
db=json.loads(Path(os.environ['M12_DIRECT_COMPILE_DB']).read_text())
link_txt=Path(os.environ['M12_DIRECT_LINK_TXT'])
entry=next((x for x in db if str(x.get('file','')).endswith('/documentdb_test.cpp')),None)
if entry is None: raise SystemExit('M12 oracle build: missing documentdb_test.cpp compile entry')
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
try: args[args.index(old)]=str(oracle)
except ValueError: raise SystemExit('M12 oracle build: source not found in compile command')
try: oi=args.index('-o')
except ValueError: raise SystemExit('M12 oracle build: compile command has no -o')
args[oi+1]=str(obj)
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
else: raise SystemExit('M12 oracle build: documentdb object not found in link command')
try: oi=link.index('-o')
except ValueError: raise SystemExit('M12 oracle build: link command has no -o')
link[oi+1]=str(out)
subprocess.run(link,cwd=str(vespa / 'searchcore/src/tests/proton/documentdb'),check=True)
