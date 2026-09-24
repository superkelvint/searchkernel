#!/usr/bin/env python3
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
vespa_root = sys.argv[2]
deps_root = sys.argv[3]
for name in ("flags.make", "link.txt"):
    for path in root.rglob(name):
        text = path.read_text(encoding="utf-8")
        text = text.replace(vespa_root, "/work/vespa")
        text = text.replace(deps_root, "/opt/vespa-deps")
        path.write_text(text, encoding="utf-8")
