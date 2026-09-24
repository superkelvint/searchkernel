#!/usr/bin/env python3
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = "pub(crate) const PREVIOUS_MANIFEST_FORMAT_VERSION: u32 = 3;"
new = "pub(crate) const PREVIOUS_MANIFEST_FORMAT_VERSION: u32 = 99;"
if text.count(old) != 1:
    raise SystemExit("could not locate unique previous-manifest compatibility constant")
path.write_text(text.replace(old, new))
