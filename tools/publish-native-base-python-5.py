#!/usr/bin/env python3
import pathlib, shlex, sys
link_file = pathlib.Path(sys.argv[1])
vespa = pathlib.Path(sys.argv[2])
deps = pathlib.Path(sys.argv[3])
text = link_file.read_text(encoding="utf-8").replace("/work/vespa", str(vespa)).replace("/opt/vespa-deps", str(deps))
cwd = link_file.parent.parent.parent
missing = []
for token in shlex.split(text):
    if not token.endswith(".a") or pathlib.Path(token).name == "libsearchcore_test.a":
        continue
    path = pathlib.Path(token)
    if not path.is_absolute():
        path = cwd / path
    if not path.exists():
        missing.append(str(path))
if missing:
    raise SystemExit("packaged static-link closure is incomplete:\n  " + "\n  ".join(missing))
