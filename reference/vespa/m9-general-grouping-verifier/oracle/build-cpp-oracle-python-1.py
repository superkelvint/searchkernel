#!/usr/bin/env python3
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
