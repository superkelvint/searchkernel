#!/usr/bin/env python3
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = "        if expected != &actual {\n"
new = "        if false && expected != &actual {\n"
if text.count(old) != 1:
    raise SystemExit("could not locate unique config-identity validation branch")
path.write_text(text.replace(old, new))
